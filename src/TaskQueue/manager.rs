use crate::TaskQueue::queue::TaskQueue;
use dashmap::DashMap;
use std::any::{Any, TypeId};
use std::sync::{Arc, OnceLock};

/// 全局任务队列管理器，通过泛型类型键管理多个独立的 `TaskQueue<T>`。
///
/// # 用法
///
/// ```rust,no_run
/// use ArcDevKit::TaskQueue::*;
///
/// struct DownloadTask { url: String }
/// struct ReportTask  { report_id: u64 }
///
/// let manager = TaskQueueManager::global();
///
/// let download_queue = manager.get_or_create::<DownloadTask>();
/// download_queue.enqueue(DownloadTask { url: "https://...".into() }).unwrap();
///
/// let report_queue = manager.get_or_create::<ReportTask>();
/// report_queue.enqueue(ReportTask { report_id: 42 }).unwrap();
/// ```
pub struct TaskQueueManager {
    queues: DashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

static GLOBAL_MANAGER: OnceLock<TaskQueueManager> = OnceLock::new();

impl TaskQueueManager {
    pub fn new() -> Self {
        Self {
            queues: DashMap::new(),
        }
    }


    pub fn global() -> &'static TaskQueueManager {
        GLOBAL_MANAGER.get_or_init(TaskQueueManager::new)
    }


    pub fn get_or_create<T: Send + Sync + 'static>(&self) -> Arc<TaskQueue<T>> {
        let type_id = TypeId::of::<T>();

        self.queues.entry(type_id).or_insert_with(|| {
            Arc::new(TaskQueue::<T>::new()) as Arc<dyn Any + Send + Sync>
        });

        let guard = self.queues.get(&type_id).expect("entry was just ensured");
        Arc::clone(guard.value())
            .downcast::<TaskQueue<T>>()
            .expect("TaskQueue type mismatch")
    }


    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<TaskQueue<T>>> {
        let type_id = TypeId::of::<T>();
        self.queues.get(&type_id).map(|guard| {
            Arc::clone(guard.value())
                .downcast::<TaskQueue<T>>()
                .expect("TaskQueue type mismatch")
        })
    }


    pub fn remove<T: Send + Sync + 'static>(&self) -> bool {
        let type_id = TypeId::of::<T>();
        if let Some((_, arc_ref)) = self.queues.remove(&type_id) {
            if let Ok(queue) = arc_ref.downcast::<TaskQueue<T>>() {
                queue.close();
            }
            true
        } else {
            false
        }
    }

    pub fn queue_count(&self) -> usize {
        self.queues.len()
    }
}

impl Default for TaskQueueManager {
    fn default() -> Self {
        Self::new()
    }
}
