// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use build_info::process::output_within;
use std::process::Command;
use std::time::{Duration, Instant};

fn cargo() -> Command {
    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
}

#[test]
fn a_command_that_finishes_hands_back_its_output() {
    let mut command = cargo();
    command.arg("--version");
    let out = output_within(command, Duration::from_secs(60)).unwrap();
    assert!(String::from_utf8(out).unwrap().starts_with("cargo "));
}

#[test]
fn a_command_that_fails_is_an_error() {
    let mut command = cargo();
    command.arg("no-such-subcommand-anywhere");
    assert!(output_within(command, Duration::from_secs(60)).is_err());
}

#[test]
fn a_command_that_hangs_is_stopped_at_the_deadline() {
    let command = if cfg!(windows) {
        let mut ping = Command::new("ping");
        ping.args(["-n", "30", "127.0.0.1"]);
        ping
    } else {
        let mut sleep = Command::new("sleep");
        sleep.arg("30");
        sleep
    };
    let started = Instant::now();
    let err = output_within(command, Duration::from_millis(300)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(20));
}
