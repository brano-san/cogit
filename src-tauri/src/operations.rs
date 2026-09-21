//! Reads the user can outrun.
//!
//! Clicking down a commit list starts a diff per commit, and the ones the user has
//! already scrolled past are answers nobody will read. Letting them run to the end costs
//! the machine and fills memory with results that go straight in the bin, so a read is
//! given a token it can be told to stop by (P1.6).
//!
//! Only reads. A mutation that stopped halfway would leave the repository in a state the
//! user never asked for; those go in the queue instead.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use parking_lot::Mutex;

/// What a running read watches to know it should stop.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Every read that can still be told to stop.
#[derive(Debug, Default)]
pub struct Cancellations {
    running: Mutex<HashMap<u32, Cancel>>,
    next: AtomicU32,
}

impl Cancellations {
    /// Registers a read and hands back its id and its token.
    ///
    /// The id is what the frontend needs to cancel it; it travels out on the first chunk
    /// so the caller has it long before the answer.
    pub fn start(&self) -> (u32, Cancel) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let cancel = Cancel::default();
        self.running.lock().insert(id, cancel.clone());
        (id, cancel)
    }

    /// `false` when there is nothing by that id: it already finished, or never ran.
    pub fn cancel(&self, id: u32) -> bool {
        let found = self.running.lock().get(&id).cloned();
        match found {
            Some(cancel) => {
                cancel.stop();
                true
            }
            None => false,
        }
    }

    /// Called when the read is over, cancelled or not. Without this the map grows for as
    /// long as the app runs.
    pub fn finish(&self, id: u32) {
        self.running.lock().remove(&id);
    }

    /// Stops everything still running, and says how many that was.
    ///
    /// Called when the app is on its way out: an answer nobody will ever read is not
    /// worth making the machine finish (problem 13).
    pub fn cancel_all(&self) -> usize {
        let held = self.running.lock();
        for cancel in held.values() {
            cancel.stop();
        }
        held.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_token_is_not_cancelled() {
        let registry = Cancellations::default();
        let (_, cancel) = registry.start();
        assert!(!cancel.is_cancelled());
    }

    #[test]
    fn cancelling_by_id_reaches_the_token_the_work_holds() {
        let registry = Cancellations::default();
        let (id, cancel) = registry.start();

        assert!(registry.cancel(id));
        assert!(cancel.is_cancelled());
    }

    #[test]
    fn ids_are_not_reused_while_the_app_runs() {
        let registry = Cancellations::default();
        let (first, _) = registry.start();
        let (second, _) = registry.start();
        assert_ne!(first, second);
    }

    #[test]
    fn cancelling_something_that_already_finished_is_not_an_error() {
        let registry = Cancellations::default();
        let (id, _) = registry.start();
        registry.finish(id);

        assert!(
            !registry.cancel(id),
            "nothing to cancel, and nothing to panic about"
        );
    }

    #[test]
    fn finishing_a_read_does_not_leave_it_behind() {
        let registry = Cancellations::default();
        let (id, _) = registry.start();
        assert_eq!(registry.running.lock().len(), 1);

        registry.finish(id);
        assert_eq!(registry.running.lock().len(), 0);
    }

    #[test]
    fn closing_stops_everything_at_once() {
        let registry = Cancellations::default();
        let (_, one) = registry.start();
        let (_, two) = registry.start();

        assert_eq!(registry.cancel_all(), 2);
        assert!(one.is_cancelled());
        assert!(two.is_cancelled());
    }

    #[test]
    fn one_cancellation_leaves_the_others_running() {
        let registry = Cancellations::default();
        let (first, one) = registry.start();
        let (_, two) = registry.start();

        registry.cancel(first);

        assert!(one.is_cancelled());
        assert!(!two.is_cancelled(), "only the read that was named stops");
    }
}
