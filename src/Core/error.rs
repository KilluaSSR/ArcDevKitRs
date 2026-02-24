use serde::{Deserialize, Serialize};
use std::fmt;

/// 模块与能力执行的全局错误
#[derive(Debug)]
pub enum ArcError {
    /// 权限不足（如需要管理员权限）
    PermissionDenied,
    /// 资源未找到
    NotFound(String),
    /// I/O 错误
    Io(std::io::Error),
    /// 操作系统 API 调用失败
    OsApi {
        code: u32,
        message: String,
    },
    /// 数据解析或转换错误
    ParseError {
        kind: String,
        detail: String,
    },
    /// 操作被跳过
    Skipped(SkipReason),
    /// 操作超时
    Timeout {
        operation: String,
        duration_secs: u64,
    },
    /// 序列化/反序列化错误
    Serialization(String),
    /// 任务队列已关闭，无法继续操作
    QueueClosed,
    /// 其他通用错误
    Other(String),
}

/// 模块或能力被跳过的原因
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkipReason {
    /// 模块被用户配置禁用
    Disabled,
    /// 需要管理员权限但当前不满足
    RequiresAdmin,
    /// 不支持当前操作系统
    UnsupportedOS,
    /// 依赖缺失
    DependencyMissing(String),
    /// 被过滤器排除
    FilteredOut,
    /// 其他原因
    Other(String),
}

impl fmt::Display for ArcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied => write!(f, "权限不足/Permission Denied"),
            Self::NotFound(res) => write!(f, "未找到资源: {}", res),
            Self::Io(e) => write!(f, "I/O 错误: {}", e),
            Self::OsApi { code, message } => write!(f, "OS API 错误 (0x{:X}): {}", code, message),
            Self::ParseError { kind, detail } => write!(f, "解析错误 ({}): {}", kind, detail),
            Self::Skipped(reason) => write!(f, "操作已跳过: {:?}", reason),
            Self::Timeout { operation, duration_secs } => write!(f, "操作超时 ({}s): {}", duration_secs, operation),
            Self::Serialization(msg) => write!(f, "序列化错误: {}", msg),
            Self::QueueClosed => write!(f, "任务队列已关闭/Task queue is closed"),
            Self::Other(msg) => write!(f, "错误: {}", msg),
        }
    }
}

impl std::error::Error for ArcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ArcError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for ArcError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e.to_string())
    }
}

impl ArcError {
    pub fn os_api(code: u32, message: impl Into<String>) -> Self {
        Self::OsApi { code, message: message.into() }
    }

    pub fn parse(kind: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::ParseError { kind: kind.into(), detail: detail.into() }
    }

    pub fn timeout(operation: impl Into<String>, duration_secs: u64) -> Self {
        Self::Timeout { operation: operation.into(), duration_secs }
    }

    pub fn is_skipped(&self) -> bool {
        matches!(self, Self::Skipped(_))
    }
}

/// 模块级错误别名
pub type ModuleError = ArcError;

/// 能力级错误别名
pub type AbilityError = ArcError;
