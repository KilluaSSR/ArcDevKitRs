use dashmap::DashMap;
use std::any::{Any, TypeId};

type Handler = Box<dyn Fn(&dyn Any) + Send + Sync>;

pub struct EventBus {
    listeners: DashMap<TypeId, Vec<Handler>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            listeners: DashMap::new(),
        }
    }

    pub fn publish<E: Any + Send + Sync>(&self, event: &E) {
        let type_id = TypeId::of::<E>();
        if let Some(handlers) = self.listeners.get(&type_id) {
            for handler in handlers.value().iter() {
                handler(event as &dyn Any);
            }
        }
    }

    pub fn subscribe<E: Any + Send + Sync, F>(&self, handler: F)
    where
        F: Fn(&E) + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<E>();
        let wrapped: Handler = Box::new(move |any| {
            if let Some(event) = any.downcast_ref::<E>() {
                handler(event);
            }
        });
        self.listeners.entry(type_id).or_default().push(wrapped);
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
