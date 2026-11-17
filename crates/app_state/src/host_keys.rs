use crate::AppEvent;
use git_engine::HostKeyQuestion;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

const POLL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyAsk {
    pub id: u32,
    pub question: HostKeyQuestion,
}

#[derive(Debug)]
struct Waiting {
    question: HostKeyQuestion,
    answer: mpsc::Sender<bool>,
}

#[derive(Debug)]
pub(crate) struct HostKeyDesk {
    next: AtomicU32,
    waiting: Mutex<BTreeMap<u32, Waiting>>,
    events: broadcast::Sender<AppEvent>,
}

impl HostKeyDesk {
    pub(crate) fn new(events: broadcast::Sender<AppEvent>) -> Self {
        Self {
            next: AtomicU32::new(1),
            waiting: Mutex::new(BTreeMap::new()),
            events,
        }
    }

    pub(crate) fn ask(
        &self,
        question: &HostKeyQuestion,
        gone: &dyn Fn() -> bool,
        limit: Duration,
    ) -> bool {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (answer, answered) = mpsc::channel();
        self.waiting.lock().insert(
            id,
            Waiting {
                question: question.clone(),
                answer,
            },
        );
        let asked = HostKeyAsk {
            id,
            question: question.clone(),
        };
        if self.events.send(AppEvent::HostKeyAsked(asked)).is_err() {
            self.waiting.lock().remove(&id);
            return false;
        }
        let started = Instant::now();
        let accepted = loop {
            match answered.recv_timeout(POLL) {
                Ok(accepted) => break Some(accepted),
                Err(mpsc::RecvTimeoutError::Disconnected) => break None,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if gone() || started.elapsed() >= limit {
                        break None;
                    }
                }
            }
        };
        if accepted.is_none() && self.waiting.lock().remove(&id).is_some() {
            let _ = self.events.send(AppEvent::HostKeyWithdrawn { id });
        }
        accepted.unwrap_or(false)
    }

    pub(crate) fn answer(&self, id: u32, accept: bool) {
        if let Some(waiting) = self.waiting.lock().remove(&id) {
            let _ = waiting.answer.send(accept);
        }
    }

    pub(crate) fn pending(&self) -> Vec<HostKeyAsk> {
        self.waiting
            .lock()
            .iter()
            .map(|(id, waiting)| HostKeyAsk {
                id: *id,
                question: waiting.question.clone(),
            })
            .collect()
    }
}

impl crate::AppState {
    pub fn ask_host_keys_here(&self) {
        let desk = Arc::clone(&self.host_keys);
        git_engine::ask_host_keys(Arc::new(move |question, gone| {
            desk.ask(question, gone, git_engine::HOST_KEY_ASK_LIMIT)
        }));
    }

    pub fn answer_host_key(&self, id: u32, accept: bool) {
        self.host_keys.answer(id, accept);
    }

    #[must_use]
    pub fn host_key_questions(&self) -> Vec<HostKeyAsk> {
        self.host_keys.pending()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn question() -> HostKeyQuestion {
        HostKeyQuestion {
            host: "h.example".to_owned(),
            address: None,
            key_type: "ED25519".to_owned(),
            fingerprint: "SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU".to_owned(),
            other_types_known: false,
        }
    }

    fn desk() -> (Arc<HostKeyDesk>, broadcast::Receiver<AppEvent>) {
        let (events, heard) = broadcast::channel(16);
        (Arc::new(HostKeyDesk::new(events)), heard)
    }

    fn asked(heard: &mut broadcast::Receiver<AppEvent>) -> u32 {
        loop {
            if let AppEvent::HostKeyAsked(ask) = heard.blocking_recv().unwrap() {
                return ask.id;
            }
        }
    }

    #[test]
    fn only_the_answer_to_its_own_question_counts() {
        let (desk, mut heard) = desk();
        let asking = Arc::clone(&desk);
        let waiting =
            std::thread::spawn(move || asking.ask(&question(), &|| false, Duration::from_secs(30)));
        let id = asked(&mut heard);
        assert_eq!(desk.pending().len(), 1);
        desk.answer(id + 1, true);
        desk.answer(id, true);
        assert!(waiting.join().unwrap());
        assert!(desk.pending().is_empty());
    }

    #[test]
    fn no_answer_in_time_or_a_gone_helper_is_a_refusal() {
        let (desk, mut heard) = desk();
        assert!(!desk.ask(&question(), &|| false, Duration::from_millis(10)));
        let id = asked(&mut heard);
        assert!(matches!(
            heard.blocking_recv().unwrap(),
            AppEvent::HostKeyWithdrawn { id: withdrawn } if withdrawn == id
        ));
        assert!(!desk.ask(&question(), &|| true, Duration::from_secs(30)));
        assert!(desk.pending().is_empty());
    }

    #[test]
    fn nobody_listening_is_a_refusal() {
        let (events, heard) = broadcast::channel(4);
        drop(heard);
        let desk = HostKeyDesk::new(events);
        assert!(!desk.ask(&question(), &|| false, Duration::from_secs(30)));
        assert!(desk.pending().is_empty());
    }
}
