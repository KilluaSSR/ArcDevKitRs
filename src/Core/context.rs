use dashmap::DashMap;
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::Core::event::EventBus;

pub struct AbilityExecutionContext {
    pub is_admin: bool,
    pub output_enabled: bool,
    pub timeout: Option<Duration>,
    pub cancellation: CancellationToken,
    pub metadata: DashMap<String, Box<dyn Any + Send + Sync>>,
    pub event_bus: Arc<EventBus>,
}

impl Default for AbilityExecutionContext {
    fn default() -> Self {
        Self {
            is_admin: false,
            timeout: None,
            cancellation: CancellationToken::new(),
            metadata: DashMap::new(),
            event_bus: Arc::new(EventBus::new()),
            output_enabled: true,
        }
    }
}

impl AbilityExecutionContext {
    pub fn with_admin(mut self) -> Self {
        self.is_admin = true;
        self
    }

    pub fn disable_output(mut self) -> Self {
        self.output_enabled = false;
        self
    }

    pub fn enable_output(mut self) -> Self {
        self.output_enabled = true;
        self
    }

    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.timeout = Some(duration);
        self
    }

    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation = token;
        self
    }

    pub fn with_event_bus(mut self, bus: Arc<EventBus>) -> Self {
        self.event_bus = bus;
        self
    }

    pub fn set_metadata<T: Any + Send + Sync>(&self, key: impl Into<String>, value: T) {
        self.metadata.insert(key.into(), Box::new(value));
    }

    pub fn with_metadata<T: Any + Send + Sync, R>(&self, key: &str, f: impl FnOnce(&T) -> R) -> Option<R> {
        self.metadata
            .get(key)
            .and_then(|v| v.value().downcast_ref::<T>().map(f))
    }
}
