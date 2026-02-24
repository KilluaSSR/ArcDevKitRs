use std::time::Duration;

/// 能力执行上下文

#[derive(Clone, Copy, Debug)]
pub struct AbilityExecutionContext {
    /// 是否以管理员权限运行
    pub is_admin: bool,
    /// 执行超时时间
    pub timeout: Option<Duration>,
}

impl AbilityExecutionContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_admin(mut self, is_admin: bool) -> Self {
        self.is_admin = is_admin;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

impl Default for AbilityExecutionContext {
    fn default() -> Self {
        Self {
            is_admin: false,
            timeout: Some(Duration::from_secs(60)),
        }
    }
}
