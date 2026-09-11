use browsai_runtime_observer::{RuntimeEvent, RuntimeObserver};

#[test]
fn runtime_observer_preserves_task_and_microtask_order() {
    let mut observer = RuntimeObserver::default();
    let task = observer.start_task();
    observer.observe(RuntimeEvent::MicrotaskQueued { task_id: task });
    observer.complete_task(task);
    let events: Vec<_> = observer.drain().collect();
    assert!(matches!(events[0], RuntimeEvent::TaskStarted { .. }));
    assert!(matches!(events[1], RuntimeEvent::MicrotaskQueued { .. }));
    assert!(matches!(events[2], RuntimeEvent::TaskCompleted { .. }));
}
