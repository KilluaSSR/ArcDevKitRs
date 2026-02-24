use ArcDevKit::TaskQueue::{TaskQueue, TaskQueueManager};

// ─── 定义任务类型（类型本身即为队列的键，无需字符串）───────────

struct DownloadTask {
    url: String,
}

struct ReportTask {
    report_id: u64,
}

struct NotifyTask {
    _message: String,
}

// ─── 基础生产-消费 ──────────────────────────────────────────────

#[tokio::test]
async fn basic_enqueue_dequeue() {
    let manager = TaskQueueManager::new();
    let queue = manager.get_or_create::<DownloadTask>();

    queue
        .enqueue(DownloadTask {
            url: "https://example.com/a".into(),
        })
        .unwrap();
    queue
        .enqueue(DownloadTask {
            url: "https://example.com/b".into(),
        })
        .unwrap();

    assert_eq!(queue.len(), 2);

    let first = queue.dequeue().await.unwrap();
    assert_eq!(first.url, "https://example.com/a");

    let second = queue.dequeue().await.unwrap();
    assert_eq!(second.url, "https://example.com/b");

    assert_eq!(queue.len(), 0);
    assert!(queue.is_empty());
}

// ─── try_dequeue 空队列 ─────────────────────────────────────────

#[tokio::test]
async fn try_dequeue_empty_returns_none() {
    let manager = TaskQueueManager::new();
    let queue = manager.get_or_create::<ReportTask>();
    assert!(queue.try_dequeue().await.is_none());
}

// ─── 多生产者并发 enqueue ───────────────────────────────────────

#[tokio::test]
async fn concurrent_producers() {
    let manager = TaskQueueManager::new();
    let queue: &'static TaskQueue<u64> = manager.get_or_create::<u64>();

    let mut handles = Vec::new();
    for i in 0..10u64 {
        handles.push(tokio::spawn(async move {
            for j in 0..100u64 {
                queue.enqueue(i * 100 + j).unwrap();
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(queue.len(), 1000);

    let mut collected = Vec::new();
    while let Some(item) = queue.try_dequeue().await {
        collected.push(item);
    }
    assert_eq!(collected.len(), 1000);
}

// ─── 队列关闭后 dequeue 返回 None ──────────────────────────────

#[tokio::test]
async fn close_then_dequeue_returns_none() {
    let manager = TaskQueueManager::new();
    let queue = manager.get_or_create::<String>();

    queue.enqueue("hello".into()).unwrap();
    queue.close_async().await;

    // 已排队的任务仍可取出
    let item = queue.dequeue().await;
    assert_eq!(item.unwrap(), "hello");

    // 队列已空且已关闭，应返回 None
    let item = queue.dequeue().await;
    assert!(item.is_none());
}

// ─── 关闭后 enqueue 返回错误 ───────────────────────────────────

#[tokio::test]
async fn enqueue_after_close_fails() {
    let manager = TaskQueueManager::new();
    let queue = manager.get_or_create::<u32>();

    queue.close_async().await;
    let result = queue.enqueue(42);
    assert!(result.is_err());
}

// ─── TaskQueueManager: get_or_create 幂等性 ────────────────────

#[tokio::test]
async fn manager_get_or_create_idempotent() {
    let manager = TaskQueueManager::new();

    let q1 = manager.get_or_create::<DownloadTask>();
    let q2 = manager.get_or_create::<DownloadTask>();

    // 应返回同一个 &'static 引用
    assert!(std::ptr::eq(q1, q2));
}

// ─── TaskQueueManager: 类型隔离 ────────────────────────────────

#[tokio::test]
async fn manager_type_isolation() {
    let manager = TaskQueueManager::new();

    let download_q = manager.get_or_create::<DownloadTask>();
    let report_q = manager.get_or_create::<ReportTask>();

    download_q
        .enqueue(DownloadTask {
            url: "https://a.com".into(),
        })
        .unwrap();
    report_q.enqueue(ReportTask { report_id: 1 }).unwrap();
    report_q.enqueue(ReportTask { report_id: 2 }).unwrap();

    assert_eq!(download_q.len(), 1);
    assert_eq!(report_q.len(), 2);
    assert_eq!(manager.queue_count(), 2);
}

// ─── TaskQueueManager: get 不存在返回 None ─────────────────────

#[tokio::test]
async fn manager_get_nonexistent_returns_none() {
    let manager = TaskQueueManager::new();
    assert!(manager.get::<NotifyTask>().is_none());
}

// ─── TaskQueueManager: remove 移除队列 ─────────────────────────

#[tokio::test]
async fn manager_remove_queue() {
    let manager = TaskQueueManager::new();
    let _q = manager.get_or_create::<ReportTask>();
    assert_eq!(manager.queue_count(), 1);

    let removed = manager.remove::<ReportTask>();
    assert!(removed);
    assert_eq!(manager.queue_count(), 0);
    assert!(manager.get::<ReportTask>().is_none());
}

// ─── TaskQueueManager: global 单例 ─────────────────────────────

#[tokio::test]
async fn manager_global_singleton() {
    let g1 = TaskQueueManager::global();
    let g2 = TaskQueueManager::global();
    assert!(std::ptr::eq(g1, g2));
}

// ─── 跨任务生产消费 ─────────────

#[tokio::test]
async fn cross_task_produce_consume() {
    let manager = TaskQueueManager::new();
    let queue: &'static TaskQueue<String> = manager.get_or_create::<String>();

    // 生产者任务 
    let producer = tokio::spawn(async move {
        for i in 0..5 {
            queue.enqueue(format!("task-{}", i)).unwrap();
        }
        queue.close_async().await;
    });

    // 因为 queue 已被 move 到 producer 中，需要重新从 manager 获取
    // 但这是同一个 &'static 引用
    let queue = manager.get_or_create::<String>();
    let consumer = tokio::spawn(async move {
        let mut results = Vec::new();
        while let Some(item) = queue.dequeue().await {
            results.push(item);
        }
        results
    });

    producer.await.unwrap();
    let results = consumer.await.unwrap();

    assert_eq!(results.len(), 5);
    assert_eq!(results[0], "task-0");
    assert_eq!(results[4], "task-4");
}
