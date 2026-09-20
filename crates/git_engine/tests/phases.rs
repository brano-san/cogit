use git_engine::phases::{Phase, PhaseTimer, bottleneck, phase_of};
use std::time::Duration;

#[test]
fn the_remote_side_of_a_fetch_is_recognised() {
    assert_eq!(
        phase_of("remote: Enumerating objects: 12, done."),
        Some(Phase::Enumerating)
    );
    assert_eq!(
        phase_of("remote: Counting objects: 100% (12/12), done."),
        Some(Phase::Counting)
    );
    assert_eq!(
        phase_of("remote: Compressing objects:  45% (7/15)"),
        Some(Phase::Compressing)
    );
}

#[test]
fn the_local_side_of_a_fetch_is_recognised() {
    assert_eq!(
        phase_of("Receiving objects:  40% (800/2000), 1.20 MiB | 600.00 KiB/s"),
        Some(Phase::Receiving)
    );
    assert_eq!(
        phase_of("Resolving deltas: 100% (500/500), done."),
        Some(Phase::Resolving)
    );
    assert_eq!(
        phase_of("Updating files:  90% (300/340)"),
        Some(Phase::UpdatingTree)
    );
    assert_eq!(
        phase_of("Writing objects: 100% (5/5), 512 bytes | 512.00 KiB/s, done."),
        Some(Phase::Writing)
    );
}

#[test]
fn an_ordinary_message_belongs_to_no_phase() {
    assert_eq!(phase_of("From github.com:brano-san/cogit"), None);
    assert_eq!(
        phase_of("   9f1aa41..d32f8d2  master -> origin/master"),
        None
    );
}

#[test]
fn everything_before_the_first_progress_line_counts_as_negotiation() {
    let mut timer = PhaseTimer::new();
    timer.observe_at(
        "From github.com:brano-san/cogit",
        Duration::from_millis(300),
    );
    timer.observe_at("Receiving objects: 10% (1/10)", Duration::from_millis(500));
    let timings = timer.finish_at(Duration::from_millis(900));

    assert_eq!(timings.of(Phase::Negotiating), Duration::from_millis(500));
    assert_eq!(timings.of(Phase::Receiving), Duration::from_millis(400));
}

#[test]
fn a_phase_accumulates_across_its_progress_updates() {
    let mut timer = PhaseTimer::new();
    timer.observe_at("Receiving objects: 10% (1/10)", Duration::from_millis(100));
    timer.observe_at("Receiving objects: 50% (5/10)", Duration::from_millis(400));
    timer.observe_at("Resolving deltas: 10% (1/10)", Duration::from_millis(700));
    let timings = timer.finish_at(Duration::from_millis(800));

    assert_eq!(timings.of(Phase::Receiving), Duration::from_millis(600));
    assert_eq!(timings.of(Phase::Resolving), Duration::from_millis(100));
}

#[test]
fn the_transferred_size_is_read_from_the_progress_line() {
    let mut timer = PhaseTimer::new();
    timer.observe_at(
        "Receiving objects: 100% (2000/2000), 3.50 MiB | 1.75 MiB/s, done.",
        Duration::from_millis(100),
    );
    let timings = timer.finish_at(Duration::from_millis(200));

    assert_eq!(timings.bytes, Some(3_670_016));
    assert_eq!(timings.objects, Some(2000));
}

#[test]
fn a_slow_transfer_blames_the_network() {
    let mut timer = PhaseTimer::new();
    timer.observe_at("Receiving objects: 1% (20/2000)", Duration::from_millis(50));
    timer.observe_at("Resolving deltas: 100% (5/5)", Duration::from_millis(9_000));
    let timings = timer.finish_at(Duration::from_millis(9_100));

    assert_eq!(bottleneck(&timings), "network transfer");
}

#[test]
fn a_slow_checkout_blames_the_working_tree() {
    let mut timer = PhaseTimer::new();
    timer.observe_at("Receiving objects: 100% (20/20)", Duration::from_millis(50));
    timer.observe_at("Updating files: 1% (10/9000)", Duration::from_millis(200));
    let timings = timer.finish_at(Duration::from_millis(8_000));

    assert_eq!(bottleneck(&timings), "local working tree update");
}

#[test]
fn a_slow_remote_blames_the_server() {
    let mut timer = PhaseTimer::new();
    timer.observe_at(
        "remote: Compressing objects: 1% (1/900)",
        Duration::from_millis(100),
    );
    timer.observe_at(
        "Receiving objects: 100% (20/20)",
        Duration::from_millis(7_000),
    );
    let timings = timer.finish_at(Duration::from_millis(7_100));

    assert_eq!(bottleneck(&timings), "remote server");
}

#[test]
fn time_outside_every_known_phase_is_ours_to_answer_for() {
    let mut timer = PhaseTimer::new();
    timer.observe_at(
        "Receiving objects: 100% (20/20)",
        Duration::from_millis(4_000),
    );
    let timings = timer.finish_at(Duration::from_millis(4_050));

    assert_eq!(bottleneck(&timings), "connection setup");
}

#[test]
fn the_summary_names_every_phase_that_took_time() {
    let mut timer = PhaseTimer::new();
    timer.observe_at("Receiving objects: 5% (1/20)", Duration::from_millis(100));
    timer.observe_at("Resolving deltas: 100% (5/5)", Duration::from_millis(400));
    let timings = timer.finish_at(Duration::from_millis(500));

    let summary = timings.summary();
    assert!(summary.contains("negotiating=100"), "{summary}");
    assert!(summary.contains("receiving=300"), "{summary}");
    assert!(summary.contains("resolving=100"), "{summary}");
}
