use crate::Core::error::ArcError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct TaskQueueConfig {
    pub rate_limit: Option<Duration>,
    pub max_retries: Option<u32>,
    pub retry_delay: Option<Duration>,
}

impl Default for TaskQueueConfig {
    fn default() -> Self {
        Self {
            rate_limit: None,
            max_retries: None,
            retry_delay: None,
        }
    }
}

pub struct TaskQueue<T: Send + 'static> {
    sender: mpsc::UnboundedSender<T>,
    receiver: tokio::sync::Mutex<mpsc::UnboundedReceiver<T>>,
    pending: AtomicUsize,
    config: tokio::sync::RwLock<TaskQueueConfig>,
    last_dequeue: tokio::sync::Mutex<Option<Instant>>,
}

impl<T: Send + 'static> TaskQueue<T> {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            sender: tx,
            receiver: tokio::sync::Mutex::new(rx),
            pending: AtomicUsize::new(0),
            config: tokio::sync::RwLock::new(TaskQueueConfig::default()),
            last_dequeue: tokio::sync::Mutex::new(None),
        }
    }

    pub fn with_config(config: TaskQueueConfig) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            sender: tx,
            receiver: tokio::sync::Mutex::new(rx),
            pending: AtomicUsize::new(0),
            config: tokio::sync::RwLock::new(config),
            last_dequeue: tokio::sync::Mutex::new(None),
        }
    }

    pub async fn set_config(&self, config: TaskQueueConfig) {
        *self.config.write().await = config;
    }

    pub fn enqueue(&self, item: T) -> Result<(), ArcError> {
        self.sender
            .send(item)
            .map_err(|_| ArcError::QueueClosed)?;
        self.pending.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub async fn try_dequeue(&self) -> Option<T> {
        if !self.check_rate_limit().await {
            return None;
        }
        let mut rx = self.receiver.lock().await;
        match rx.try_recv() {
            Ok(item) => {
                self.pending.fetch_sub(1, Ordering::Relaxed);
                self.mark_dequeued().await;
                Some(item)
            }
            Err(_) => None,
        }
    }

    pub async fn dequeue(&self) -> Option<T> {
        self.wait_rate_limit().await;
        let mut rx = self.receiver.lock().await;
        let item = rx.recv().await;
        if item.is_some() {
            self.pending.fetch_sub(1, Ordering::Relaxed);
            self.mark_dequeued().await;
        }
        item
    }

    pub fn close(&self) {
        if let Ok(mut rx) = self.receiver.try_lock() {
            rx.close();
        }
    }

    pub async fn close_async(&self) {
        let mut rx = self.receiver.lock().await;
        rx.close();
    }

    pub fn len(&self) -> usize {
        self.pending.load(Ordering::Relaxed)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }

    /// 非阻塞检查速率限制，返回 true 表示可以消费
    async fn check_rate_limit(&self) -> bool {
        let rate_limit = { self.config.read().await.rate_limit };
        if let Some(rate_limit) = rate_limit {
            let last = { *self.last_dequeue.lock().await };
            if let Some(last_time) = last {
                return last_time.elapsed() >= rate_limit;
            }
        }
        true
    }

    /// 阻塞等待直到速率限制允许
    async fn wait_rate_limit(&self) {
        let rate_limit = { self.config.read().await.rate_limit };
        if let Some(rate_limit) = rate_limit {
            let last = { *self.last_dequeue.lock().await };
            if let Some(last_time) = last {
                let elapsed = last_time.elapsed();
                if elapsed < rate_limit {
                    tokio::time::sleep(rate_limit - elapsed).await;
                }
            }
        }
    }

    async fn mark_dequeued(&self) {
        *self.last_dequeue.lock().await = Some(Instant::now());
    }
}

impl<T: Send + 'static> Default for TaskQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}
