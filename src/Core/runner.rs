use crate::Core::ability::{AbilityExecutionMode, AbilityExecutor, AbilityIdentity};
use crate::Core::context::AbilityExecutionContext;
use crate::Core::error::{AbilityError, ModuleError};
use crate::Core::module::{ModuleExecutor, ModuleIdentity};
use erased_serde::Serialize as ErasedSerialize;
use std::any::Any;
use std::collections::HashMap;

pub struct AbilityResult {
    pub ability_identity: AbilityIdentity,
    pub module_identity: ModuleIdentity,
    pub display_name: &'static str,
    pub description: &'static str,
    pub output_disabled: bool,
    result: AbilityOutcome,
}

enum AbilityOutcome {
    Success {
        serializable: Box<dyn ErasedSerialize + Send + Sync>,
        any: Box<dyn Any + Send + Sync>,
    },
    Failure(AbilityError),
}

impl AbilityResult {
    pub fn is_success(&self) -> bool {
        matches!(self.result, AbilityOutcome::Success { .. })
    }

    pub fn is_skipped(&self) -> bool {
        matches!(self.result, AbilityOutcome::Failure(ref e) if e.is_skipped())
    }

    pub fn error(&self) -> Option<&AbilityError> {
        match &self.result {
            AbilityOutcome::Failure(e) => Some(e),
            _ => None,
        }
    }

    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        match &self.result {
            AbilityOutcome::Success { any, .. } => any.downcast_ref::<T>(),
            _ => None,
        }
    }

    pub fn as_serializable(&self) -> Option<&(dyn ErasedSerialize + Send + Sync)> {
        match &self.result {
            AbilityOutcome::Success { serializable, .. } => Some(serializable.as_ref()),
            _ => None,
        }
    }

    pub fn to_json(&self) -> Option<Result<String, String>> {
        self.as_serializable().map(|s| {
            let mut buf = Vec::new();
            let mut serializer = serde_json::Serializer::pretty(&mut buf);
            s.erased_serialize(&mut <dyn erased_serde::Serializer>::erase(&mut serializer))
                .map_err(|e| e.to_string())?;
            String::from_utf8(buf).map_err(|e| e.to_string())
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionStrategy {
    #[default]
    Parallel,
    Sequential,
    PriorityGrouped,
}

pub struct ModuleRegistry;

impl ModuleRegistry {
    pub fn modules() -> impl Iterator<Item = &'static dyn ModuleExecutor> {
        inventory::iter::<&'static dyn ModuleExecutor>
            .into_iter()
            .copied()
    }

    pub fn abilities() -> impl Iterator<Item = &'static dyn AbilityExecutor> {
        inventory::iter::<&'static dyn AbilityExecutor>
            .into_iter()
            .copied()
    }

    pub async fn initialize_all() -> Vec<Result<ModuleIdentity, ModuleError>> {
        let active_modules = Self::modules().filter(|m| !m.descriptor().is_disabled);

        let mut futures = Vec::new();
        for module in active_modules {
            let identity = module.descriptor().identity;
            let future = async move {
                match module.initialize_erased().await {
                    Ok(_) => Ok(identity),
                    Err(e) => Err(e),
                }
            };
            futures.push(future);
        }

        futures::future::join_all(futures).await
    }

    pub async fn shutdown_all() {
        let modules: Vec<_> = Self::modules()
            .filter(|m| !m.descriptor().is_disabled)
            .collect();
        for module in modules.into_iter().rev() {
            module.shutdown_erased().await;
        }
    }

    pub async fn execute_abilities(
        module_id: &ModuleIdentity,
        mode: AbilityExecutionMode,
        ctx: &AbilityExecutionContext,
        strategy: ExecutionStrategy,
    ) -> Vec<AbilityResult> {
        let mut targets: Vec<_> = Self::abilities()
            .filter(|a| {
                a.module_identity() == *module_id
                    && a.is_enabled()
                    && a.descriptor().execution_mode == mode
            })
            .collect();
        targets.sort_by_key(|a| a.descriptor().priority);

        let mut results = Self::run_with_strategy(targets, ctx, strategy).await;


        for result in &mut results {
            if Self::is_ability_output_disabled(result, ctx) {
                result.output_disabled = true;
            }
        }

        results
    }

    pub async fn execute_all(
        mode: AbilityExecutionMode,
        ctx: &AbilityExecutionContext,
        strategy: ExecutionStrategy,
    ) -> HashMap<ModuleIdentity, Vec<AbilityResult>> {
        let active_modules: Vec<_> = Self::modules()
            .filter(|m| !m.descriptor().is_disabled)
            .collect();

        let mut map: HashMap<ModuleIdentity, Vec<AbilityResult>> = HashMap::new();


        for module in &active_modules {
            let m_id = module.descriptor().identity;
            let mut group: Vec<_> = Self::abilities()
                .filter(|a| {
                    a.module_identity() == m_id
                        && a.is_enabled()
                        && a.descriptor().execution_mode == mode
                })
                .collect();
            group.sort_by_key(|a| a.descriptor().priority);

            let mut results = Self::run_with_strategy(group, ctx, strategy).await;

            let module_output_disabled = module.descriptor().force_disabled_output
                || module.descriptor().auto_disable_output
                || !ctx.output_enabled;


            for result in &mut results {
                if module_output_disabled || Self::is_ability_output_disabled(result, ctx) {
                    result.output_disabled = true;
                }
            }

            if !results.is_empty() {
                map.insert(m_id, results);
            }
        }

        map
    }

    fn is_ability_output_disabled(result: &AbilityResult, ctx: &AbilityExecutionContext) -> bool {
        let ability_disabled = Self::abilities()
            .find(|a| a.descriptor().identity == result.ability_identity
                && a.module_identity() == result.module_identity)
            .map(|a| {
                let desc = a.descriptor();
                desc.force_disabled_output || desc.auto_disable_output
            })
            .unwrap_or(false);

        ability_disabled || !ctx.output_enabled
    }

    async fn run_with_strategy(
        abilities: Vec<&'static dyn AbilityExecutor>,
        ctx: &AbilityExecutionContext,
        strategy: ExecutionStrategy,
    ) -> Vec<AbilityResult> {
        match strategy {
            ExecutionStrategy::Parallel => Self::run_parallel(abilities, ctx).await,
            ExecutionStrategy::Sequential => Self::run_sequential(abilities, ctx).await,
            ExecutionStrategy::PriorityGrouped => {
                Self::run_priority_grouped(abilities, ctx).await
            }
        }
    }

    async fn run_parallel(
        abilities: Vec<&'static dyn AbilityExecutor>,
        ctx: &AbilityExecutionContext,
    ) -> Vec<AbilityResult> {
        let futures: Vec<_> = abilities
            .into_iter()
            .map(|a| async move { Self::execute_single(a, ctx).await })
            .collect();
        futures::future::join_all(futures).await
    }

    async fn run_sequential(
        abilities: Vec<&'static dyn AbilityExecutor>,
        ctx: &AbilityExecutionContext,
    ) -> Vec<AbilityResult> {
        let mut results = Vec::with_capacity(abilities.len());
        for ability in abilities {
            results.push(Self::execute_single(ability, ctx).await);
        }
        results
    }

    async fn run_priority_grouped(
        abilities: Vec<&'static dyn AbilityExecutor>,
        ctx: &AbilityExecutionContext,
    ) -> Vec<AbilityResult> {
        let mut results = Vec::new();
        let mut group: Vec<&'static dyn AbilityExecutor> = Vec::new();
        let mut current_priority: Option<i32> = None;

        for ability in abilities {
            let p = ability.descriptor().priority;
            if current_priority != Some(p) {
                if !group.is_empty() {
                    let batch = Self::run_parallel(std::mem::take(&mut group), ctx).await;
                    results.extend(batch);
                }
                current_priority = Some(p);
            }
            group.push(ability);
        }

        if !group.is_empty() {
            let batch = Self::run_parallel(group, ctx).await;
            results.extend(batch);
        }

        results
    }

    async fn execute_single(
        ability: &'static dyn AbilityExecutor,
        ctx: &AbilityExecutionContext,
    ) -> AbilityResult {
        let desc = ability.descriptor();

        let outcome = if let Some(timeout) = ctx.timeout {
            match tokio::time::timeout(timeout, ability.execute(ctx)).await {
                Ok(Ok((serializable, any))) => AbilityOutcome::Success { serializable, any },
                Ok(Err(e)) => AbilityOutcome::Failure(e),
                Err(_) => AbilityOutcome::Failure(AbilityError::timeout(
                    format!("执行能力 {}", desc.display_name),
                    timeout.as_secs(),
                )),
            }
        } else {
            match ability.execute(ctx).await {
                Ok((serializable, any)) => AbilityOutcome::Success { serializable, any },
                Err(e) => AbilityOutcome::Failure(e),
            }
        };

        AbilityResult {
            ability_identity: desc.identity,
            module_identity: ability.module_identity(),
            display_name: desc.display_name,
            description: desc.description,
            output_disabled: false,
            result: outcome,
        }
    }
}
