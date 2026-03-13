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

#[derive(Debug, Copy, Clone)]
pub struct ModuleDescriptor {
    pub identity: ModuleIdentity,
    pub display_name: &'static str,
    pub description: &'static str,
    pub version: &'static str,
    pub author: Option<&'static str>,
    /// `true` 时模块完全跳过（不初始化、不执行）。
    pub is_disabled: bool,
    /// 强制屏蔽输出（模块仍执行，结果不进入结果集）。
    pub force_disabled_output: bool,
    /// 运行时动态屏蔽输出。
    pub auto_disable_output: bool,
}


pub trait Module: Send + Sync {
    fn descriptor(&self) -> &ModuleDescriptor;

    fn initialize<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<(), ModuleError>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }

    fn shutdown<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async {})
    }
}


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

inventory::collect!(&'static dyn ModuleExecutor);
