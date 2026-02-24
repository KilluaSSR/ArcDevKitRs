use crate::Core::context::AbilityExecutionContext;
use crate::Core::error::{AbilityError, SkipReason};
use crate::Core::module::ModuleIdentity;
use erased_serde::Serialize as ErasedSerialize;
use serde::Serialize;
use std::any::Any;
use std::future::Future;
use std::pin::Pin;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbilityIdentity(pub &'static str);

impl AbilityIdentity {
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AbilityExecutionMode {
    #[default]
    Auto,
    Manual,
}

#[derive(Debug, Clone, Copy)]
pub struct AbilityDescriptor {
    pub identity: AbilityIdentity,
    pub display_name: &'static str,
    pub description: &'static str,
    /// 优先级，数值越低越先执行
    pub priority: i32,
    /// 发现后是否默认挂载（禁用状态将被跳过）
    pub is_enabled_by_default: bool,
    pub execution_mode: AbilityExecutionMode,
}


pub trait Ability: Send + Sync {
    
    type Output: 'static + Send + Sync + Serialize;

    fn module_identity(&self) -> ModuleIdentity;

    fn descriptor(&self) -> &AbilityDescriptor;

    fn is_enabled(&self) -> bool {
        self.descriptor().is_enabled_by_default
    }

    fn before_execute<'a>(
        &'a self,
        _ctx: &'a AbilityExecutionContext,
    ) -> Pin<Box<dyn Future<Output = Result<bool, AbilityError>> + Send + 'a>> {
        Box::pin(async { Ok(true) })
    }

    fn run_async<'a>(
        &'a self,
        ctx: &'a AbilityExecutionContext,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output, AbilityError>> + Send + 'a>>;

    fn after_execute<'a>(
        &'a self,
        _ctx: &'a AbilityExecutionContext,
        _output: Option<&'a Self::Output>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async {})
    }

    fn on_error<'a>(
        &'a self,
        _ctx: &'a AbilityExecutionContext,
        _error: &'a AbilityError,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async {})
    }
}


pub trait AbilityExecutor: Send + Sync {
    fn module_identity(&self) -> ModuleIdentity;
    fn descriptor(&self) -> &AbilityDescriptor;
    fn is_enabled(&self) -> bool;

    fn execute_erased<'a>(
        &'a self,
        ctx: &'a AbilityExecutionContext,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Box<dyn ErasedSerialize + Send + Sync>, AbilityError>>
                + Send
                + 'a,
        >,
    >;

    fn execute_any<'a>(
        &'a self,
        ctx: &'a AbilityExecutionContext,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn Any + Send + Sync>, AbilityError>> + Send + 'a>>;
}

impl<T: Ability> AbilityExecutor for T {
    fn module_identity(&self) -> ModuleIdentity {
        self.module_identity()
    }
    
    fn descriptor(&self) -> &AbilityDescriptor {
        self.descriptor()
    }
    
    fn is_enabled(&self) -> bool {
        self.is_enabled()
    }

    fn execute_erased<'a>(
        &'a self,
        ctx: &'a AbilityExecutionContext,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Box<dyn ErasedSerialize + Send + Sync>, AbilityError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            if !self.before_execute(ctx).await? {
                return Err(AbilityError::Skipped(SkipReason::Disabled));
            }
            match self.run_async(ctx).await {
                Ok(output) => {
                    self.after_execute(ctx, Some(&output)).await;
                    Ok(Box::new(output) as Box<dyn ErasedSerialize + Send + Sync>)
                }
                Err(e) => {
                    self.on_error(ctx, &e).await;
                    Err(e)
                }
            }
        })
    }

    fn execute_any<'a>(
        &'a self,
        ctx: &'a AbilityExecutionContext,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn Any + Send + Sync>, AbilityError>> + Send + 'a>> {
        Box::pin(async move {
            if !self.before_execute(ctx).await? {
                return Err(AbilityError::Skipped(SkipReason::Disabled));
            }
            match self.run_async(ctx).await {
                Ok(output) => {
                    self.after_execute(ctx, Some(&output)).await;
                    Ok(Box::new(output) as Box<dyn Any + Send + Sync>)
                }
                Err(e) => {
                    self.on_error(ctx, &e).await;
                    Err(e)
                }
            }
        })
    }
}

// 自动向全局系统注册该 trait 对象能力
inventory::collect!(&'static dyn AbilityExecutor);
