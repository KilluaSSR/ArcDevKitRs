use crate::Core::ability::{AbilityExecutionMode, AbilityExecutor, AbilityIdentity};
use crate::Core::context::AbilityExecutionContext;
use crate::Core::error::{AbilityError, ModuleError};
use crate::Core::module::{ModuleExecutor, ModuleIdentity};
use erased_serde::Serialize as ErasedSerialize;
use std::any::Any;
use std::collections::HashMap;

/// 执行结果
pub struct AbilityResult {
    pub ability_identity: AbilityIdentity,
    pub module_identity: ModuleIdentity,
    pub display_name: &'static str,
    pub description: &'static str,
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
            let identity = module.descriptor().identity.clone();
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

    pub async fn execute_abilities(
        module_id: &ModuleIdentity,
        mode: AbilityExecutionMode,
        ctx: &AbilityExecutionContext,
    ) -> Vec<AbilityResult> {
        let targets = Self::abilities()
            .filter(|a| {
                a.module_identity() == *module_id
                    && a.is_enabled()
                    && a.descriptor().execution_mode == mode
            })
            .collect::<Vec<_>>();

        Self::run_abilities_parallel(targets, ctx).await
    }

    pub async fn execute_all(
        mode: AbilityExecutionMode,
        ctx: &AbilityExecutionContext,
    ) -> HashMap<ModuleIdentity, Vec<AbilityResult>> {
        let active_modules: Vec<_> = Self::modules()
            .filter(|m| !m.descriptor().is_disabled)
            .map(|m| m.descriptor().identity.clone())
            .collect();

        let mut target_abilities = Vec::new();
        for m_id in &active_modules {
            let mut group = Self::abilities()
                .filter(|a| {
                    a.module_identity() == *m_id
                        && a.is_enabled()
                        && a.descriptor().execution_mode == mode
                })
                .collect::<Vec<_>>();
            target_abilities.append(&mut group);
        }

        let results = Self::run_abilities_parallel(target_abilities, ctx).await;

        let mut map: HashMap<ModuleIdentity, Vec<AbilityResult>> = HashMap::new();
        for res in results {
            map.entry(res.module_identity.clone()).or_default().push(res);
        }
        map
    }

    async fn run_abilities_parallel(
        abilities: Vec<&'static dyn AbilityExecutor>,
        ctx: &AbilityExecutionContext,
    ) -> Vec<AbilityResult> {
        let futures: Vec<_> = abilities
            .into_iter()
            .map(|ability| async move { Self::execute_single_ability(ability, ctx).await })
            .collect();

        futures::future::join_all(futures).await
    }

    async fn execute_single_ability(
        ability: &'static dyn AbilityExecutor,
        ctx: &AbilityExecutionContext,
    ) -> AbilityResult {
        let desc = ability.descriptor();
        let execute_future = async {
            let serializable = ability.execute_erased(ctx).await?;
            let any = ability.execute_any(ctx).await?;
            Ok::<_, AbilityError>((serializable, any))
        };

        let outcome = if let Some(timeout) = ctx.timeout {
            match tokio::time::timeout(timeout, execute_future).await {
                Ok(Ok((serializable, any))) => AbilityOutcome::Success { serializable, any },
                Ok(Err(e)) => AbilityOutcome::Failure(e),
                Err(_) => AbilityOutcome::Failure(AbilityError::timeout(
                    format!("执行能力 {}", desc.display_name),
                    timeout.as_secs(),
                )),
            }
        } else {
            match execute_future.await {
                Ok((serializable, any)) => AbilityOutcome::Success { serializable, any },
                Err(e) => AbilityOutcome::Failure(e),
            }
        };

        AbilityResult {
            ability_identity: desc.identity.clone(),
            module_identity: ability.module_identity(),
            display_name: desc.display_name,
            description: desc.description,
            result: outcome,
        }
    }
}
