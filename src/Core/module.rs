use crate::Core::error::ModuleError;
use std::future::Future;
use std::pin::Pin;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleIdentity(pub &'static str);

impl ModuleIdentity {
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModuleDescriptor {
    /// 模块唯一标识
    pub identity: ModuleIdentity,
    /// 展示名称
    pub display_name: &'static str,
    /// 功能描述
    pub description: &'static str,
    /// 模块的语法版本
    pub version: &'static str,
    /// 作者
    pub author: Option<&'static str>,
    /// 是否直接被系统禁用，设为 true 时将在收集期被直接跳过
    pub is_disabled: bool,
}

/// 所有业务模块必须实现的核心 Trait，以支持系统自动收集。
pub trait Module: Send + Sync {
    fn descriptor(&self) -> &ModuleDescriptor;

    fn initialize<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<(), ModuleError>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }

    fn shutdown<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async {})
    }
}

/// 类型擦除的模块执行器。
pub trait ModuleExecutor: Send + Sync {
    fn descriptor(&self) -> &ModuleDescriptor;
    fn initialize_erased<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<(), ModuleError>> + Send + 'a>>;
    fn shutdown_erased<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

impl<T: Module> ModuleExecutor for T {
    fn descriptor(&self) -> &ModuleDescriptor {
        self.descriptor()
    }
    
    fn initialize_erased<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<(), ModuleError>> + Send + 'a>> {
        self.initialize()
    }
    
    fn shutdown_erased<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        self.shutdown()
    }
}

// 自动向全局系统注册该 trait 对象收集功能
inventory::collect!(&'static dyn ModuleExecutor);
