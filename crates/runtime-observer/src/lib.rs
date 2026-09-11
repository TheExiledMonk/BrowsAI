use browsai_provenance::{ProvenanceSource, SourceKind};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RuntimeEvent {
    TaskStarted { task_id: u64 },
    TaskCompleted { task_id: u64 },
    MicrotaskQueued { task_id: u64 },
    PromiseSettled { promise_id: u64, fulfilled: bool },
    WorkerStarted { worker_id: u64 },
    WorkerStopped { worker_id: u64 },
    Exception { message: String },
    DomMutation { node_id: u64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeEvidence {
    pub event: RuntimeEvent,
    pub provenance: Vec<ProvenanceSource>,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeObserver {
    next_task: u64,
    next_worker: u64,
    events: VecDeque<RuntimeEvidence>,
}

impl RuntimeObserver {
    pub fn start_task(&mut self) -> u64 {
        self.next_task += 1;
        let id = self.next_task;
        self.observe(RuntimeEvent::TaskStarted { task_id: id });
        id
    }
    pub fn complete_task(&mut self, task_id: u64) {
        self.observe(RuntimeEvent::TaskCompleted { task_id });
    }

    pub fn queue_microtask(&mut self, task_id: u64) {
        self.observe(RuntimeEvent::MicrotaskQueued { task_id });
    }

    pub fn settle_promise(&mut self, promise_id: u64, fulfilled: bool) {
        self.observe(RuntimeEvent::PromiseSettled {
            promise_id,
            fulfilled,
        });
    }

    pub fn start_worker(&mut self) -> u64 {
        self.next_worker += 1;
        let id = self.next_worker;
        self.observe(RuntimeEvent::WorkerStarted { worker_id: id });
        id
    }

    pub fn stop_worker(&mut self, worker_id: u64) {
        self.observe(RuntimeEvent::WorkerStopped { worker_id });
    }

    pub fn observe_exception(&mut self, message: impl Into<String>) {
        self.observe(RuntimeEvent::Exception {
            message: message.into(),
        });
    }

    pub fn observe_dom_mutation(&mut self, node_id: u64) {
        self.observe(RuntimeEvent::DomMutation { node_id });
    }

    pub fn drain_bounded(&mut self, limit: usize) -> Vec<RuntimeEvent> {
        self.events
            .drain(..limit)
            .map(|evidence| evidence.event)
            .collect()
    }

    pub fn observe(&mut self, event: RuntimeEvent) {
        self.observe_with_provenance(
            event,
            ProvenanceSource {
                kind: SourceKind::JavaScript,
                reference: "runtime-observer".into(),
                detail: None,
            },
        );
    }

    pub fn observe_with_provenance(&mut self, event: RuntimeEvent, source: ProvenanceSource) {
        self.events.push_back(RuntimeEvidence {
            event,
            provenance: vec![source],
        });
    }
    pub fn drain(&mut self) -> impl Iterator<Item = RuntimeEvent> + '_ {
        self.events.drain(..).map(|evidence| evidence.event)
    }
    pub fn drain_evidence(&mut self) -> impl Iterator<Item = RuntimeEvidence> + '_ {
        self.events.drain(..)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_helpers_preserve_order_and_bound_replay() {
        let mut observer = RuntimeObserver::default();
        let task = observer.start_task();
        observer.queue_microtask(task);
        observer.settle_promise(7, true);
        let worker = observer.start_worker();
        observer.observe_dom_mutation(42);
        observer.observe_exception("boom");
        observer.stop_worker(worker);
        observer.complete_task(task);

        let first = observer.drain_bounded(3);
        assert_eq!(first.len(), 3);
        assert!(matches!(first[0], RuntimeEvent::TaskStarted { .. }));
        assert!(matches!(first[2], RuntimeEvent::PromiseSettled { .. }));
        let rest: Vec<_> = observer.drain().collect();
        assert!(matches!(rest[0], RuntimeEvent::WorkerStarted { .. }));
        assert!(matches!(
            rest.last(),
            Some(RuntimeEvent::TaskCompleted { .. })
        ));
    }
}
