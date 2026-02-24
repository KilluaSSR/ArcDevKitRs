use crate::TaskQueue::queue::TaskQueue;
use dashmap::DashMap;
use std::any::{Any, TypeId};
use std::sync::OnceLock;

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
    queues: DashMap<TypeId, &'static (dyn Any + Send + Sync)>,
}

static GLOBAL_MANAGER: OnceLock<TaskQueueManager> = OnceLock::new();

impl TaskQueueManager {
    pub fn new() -> Self {
        Self {
            queues: DashMap::new(),
        }
    }

    /// 获取全局单例。
    pub fn global() -> &'static TaskQueueManager {
        GLOBAL_MANAGER.get_or_init(TaskQueueManager::new)
    }

    /// 获取或创建类型 `T` 对应的队列，返回 `&'static` 引用。
    pub fn get_or_create<T: Send + 'static>(&self) -> &'static TaskQueue<T> {
        let type_id = TypeId::of::<T>();

        self.queues.entry(type_id).or_insert_with(|| {
            let queue = TaskQueue::<T>::new();
            let leaked: &'static TaskQueue<T> = Box::leak(Box::new(queue));
            leaked as &'static (dyn Any + Send + Sync)
        });

        let guard = self.queues.get(&type_id).expect("entry was just ensured");
        let static_ref: &'static (dyn Any + Send + Sync) = *guard.value();
        static_ref
            .downcast_ref::<TaskQueue<T>>()
            .expect("TaskQueue type mismatch")
    }

    /// 获取类型 `T` 对应的队列，不存在返回 `None`。
    pub fn get<T: Send + 'static>(&self) -> Option<&'static TaskQueue<T>> {
        let type_id = TypeId::of::<T>();
        self.queues.get(&type_id).map(|guard| {
            let static_ref: &'static (dyn Any + Send + Sync) = *guard.value();
            static_ref
                .downcast_ref::<TaskQueue<T>>()
                .expect("TaskQueue type mismatch")
        })
    }

    /// 移除并关闭类型 `T` 对应的队列。
    pub fn remove<T: Send + 'static>(&self) -> bool {
        let type_id = TypeId::of::<T>();
        if let Some((_, static_ref)) = self.queues.remove(&type_id) {
            if let Some(queue) = static_ref.downcast_ref::<TaskQueue<T>>() {
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
