use crate::Core::error::ArcError;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc;

/// 异步生产者-消费者任务队列。
pub struct TaskQueue<T: Send + 'static> {
    sender: mpsc::UnboundedSender<T>,
    receiver: tokio::sync::Mutex<mpsc::UnboundedReceiver<T>>,
    pending: AtomicUsize,
}

impl<T: Send + 'static> TaskQueue<T> {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            sender: tx,
            receiver: tokio::sync::Mutex::new(rx),
            pending: AtomicUsize::new(0),
        }
    }

    /// 入队（非阻塞）。队列已关闭时返回 `ArcError::QueueClosed`。
    pub fn enqueue(&self, item: T) -> Result<(), ArcError> {
        self.sender
            .send(item)
            .map_err(|_| ArcError::QueueClosed)?;
        self.pending.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// 尝试出队（非阻塞），队列为空时返回 `None`。
    pub async fn try_dequeue(&self) -> Option<T> {
        let mut rx = self.receiver.lock().await;
        match rx.try_recv() {
            Ok(item) => {
                self.pending.fetch_sub(1, Ordering::Relaxed);
                Some(item)
            }
            Err(_) => None,
        }
    }

    /// 异步等待出队。生产端全部关闭且队列为空时返回 `None`。
    pub async fn dequeue(&self) -> Option<T> {
        let mut rx = self.receiver.lock().await;
        let item = rx.recv().await;
        if item.is_some() {
            self.pending.fetch_sub(1, Ordering::Relaxed);
        }
        item
    }

    /// 同步关闭队列。若消费者持有锁，改用 [`close_async`]。
    pub fn close(&self) {
        if let Ok(mut rx) = self.receiver.try_lock() {
            rx.close();
        }
    }

    /// 异步关闭队列。已排队任务仍可被消费。
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
}

impl<T: Send + 'static> Default for TaskQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}
