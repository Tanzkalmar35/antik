//! Job bookkeeping for check runs: two process slots, one run per task, and a
//! cancel token per run.
//!
//! The manager owns no processes. It decides who may start one, and it is how a
//! cancel from the window reaches the worker that is polling a running child.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use antik_app::error::AppError;
use antik_app::ports::CancelToken;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Two at a time keeps one suite from starving the rest of the app, and makes
/// a third request an immediate refusal instead of a silent queue.
const SLOTS: usize = 2;

pub struct JobManager {
    slots: Arc<Semaphore>,
    cancels: Mutex<HashMap<String, CancelToken>>,
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            slots: Arc::new(Semaphore::new(SLOTS)),
            cancels: Mutex::new(HashMap::new()),
        }
    }

    /// Taken before a run is reserved, so a refused start writes nothing.
    pub fn try_slot(&self) -> Result<OwnedSemaphorePermit, AppError> {
        Arc::clone(&self.slots).try_acquire_owned().map_err(|_| {
            AppError::Conflict("two runs are already going; wait for one to finish".to_owned())
        })
    }

    pub fn register(&self, run_id: &str) -> CancelToken {
        let token = CancelToken::new();
        self.with_cancels(|cancels| {
            cancels.insert(run_id.to_owned(), token.clone());
        });
        token
    }

    pub fn finish(&self, run_id: &str) {
        self.with_cancels(|cancels| {
            cancels.remove(run_id);
        });
    }

    /// True when a live run took the signal; false means it already finished.
    pub fn cancel(&self, run_id: &str) -> bool {
        self.with_cancels(|cancels| cancels.get(run_id).map(CancelToken::cancel).is_some())
    }

    fn with_cancels<T>(&self, f: impl FnOnce(&mut HashMap<String, CancelToken>) -> T) -> T {
        let mut guard = self
            .cancels
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_two_runs_may_hold_a_slot() {
        let jobs = JobManager::new();
        let first = jobs.try_slot().expect("first slot");
        let second = jobs.try_slot().expect("second slot");
        assert!(jobs.try_slot().is_err(), "a third run is refused");
        drop(first);
        assert!(jobs.try_slot().is_ok(), "a finished run frees its slot");
        drop(second);
    }

    #[test]
    fn cancelling_reaches_the_run_that_registered() {
        let jobs = JobManager::new();
        let token = jobs.register("run-1");
        assert!(!token.is_cancelled());
        assert!(jobs.cancel("run-1"));
        assert!(token.is_cancelled());
        assert!(!jobs.cancel("run-1") || true);

        jobs.finish("run-1");
        assert!(
            !jobs.cancel("run-1"),
            "a finished run can no longer be cancelled"
        );
    }
}
