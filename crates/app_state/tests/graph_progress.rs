// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! How often a walk reports how far it got (INV-09): the pacing is decided here, the IPC
//! layer only forwards what it is handed.

use app_state::{GraphProgress, throttled};
use std::time::Duration;

fn step(total: u32, is_last: bool) -> GraphProgress {
    GraphProgress {
        generation: 1,
        total,
        is_last,
        base: None,
        kept: 0,
    }
}

#[test]
fn the_first_and_the_last_report_go_and_those_between_wait_their_turn() {
    let mut sent = Vec::new();
    {
        let mut send = throttled(Duration::from_secs(3600), |progress| {
            sent.push(progress.total);
            true
        });
        for total in [10, 20, 30] {
            assert!(
                send(step(total, false)),
                "a report held back does not stop the walk"
            );
        }
        assert!(send(step(40, true)));
    }
    assert_eq!(sent, vec![10, 40]);
}

#[test]
fn with_no_wait_every_report_goes() {
    let mut sent = Vec::new();
    {
        let mut send = throttled(Duration::ZERO, |progress| {
            sent.push(progress.total);
            true
        });
        for total in [10, 20, 30] {
            send(step(total, false));
        }
    }
    assert_eq!(sent, vec![10, 20, 30]);
}

#[test]
fn a_report_nobody_takes_stops_the_walk() {
    let mut send = throttled(Duration::ZERO, |_| false);
    assert!(!send(step(10, false)));
}
