// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use app_state::{GraphWindow, graph_wire};
use git_engine::CommitRow;
use graph_engine::{GraphRow, NodeKind, Segment, Span};

/// The same bytes are decoded by `frontend/src/lib/graph-wire.test.ts`.
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../frontend/src/lib/graph-wire.golden.bin"
);

fn commit(oid: char, summary: &str, name: &str, timestamp: i64, zone: i32) -> CommitRow {
    CommitRow {
        oid: oid.to_string().repeat(40),
        parents: vec!["f".repeat(40)],
        summary: summary.to_owned(),
        author_name: name.to_owned(),
        author_email: format!("{}@example.com", name.to_lowercase()),
        timestamp,
        tz_offset_minutes: zone,
    }
}

fn segment(from: u16, to: u16, span: Span, primary: bool, arrow: bool) -> Segment {
    Segment {
        from,
        to,
        span,
        primary,
        color: from.try_into().unwrap(),
        arrow,
    }
}

fn sample() -> GraphWindow {
    GraphWindow {
        start: 40,
        total: 1000,
        complete: false,
        commits: vec![
            commit('a', "Merge branch 'feature'", "Ann", 1_700_000_000, 180),
            commit('b', "Übersicht — 修正", "Bo", 1_600_000_000, -300),
        ],
        rows: vec![
            GraphRow {
                row: 40,
                lane: 0,
                color: 0,
                kind: NodeKind::Merge,
                primary: true,
                width: 2,
                segments: vec![
                    segment(0, 0, Span::Bottom, true, false),
                    segment(0, 1, Span::Bottom, false, false),
                ],
            },
            GraphRow {
                row: 41,
                lane: 1,
                color: 3,
                kind: NodeKind::Root,
                primary: false,
                width: 2,
                segments: vec![segment(1, 1, Span::Top, false, true)],
            },
        ],
    }
}

#[test]
fn the_frontend_decodes_the_same_bytes() {
    let bytes = graph_wire::encode(&sample());
    if std::env::var_os("COGIT_BLESS").is_some() {
        std::fs::write(GOLDEN, &bytes).unwrap();
    }
    assert_eq!(
        bytes,
        std::fs::read(GOLDEN).unwrap(),
        "the format changed: bless with COGIT_BLESS=1 and update graph-wire.test.ts"
    );
}

#[test]
fn the_buffer_is_as_long_as_its_columns() {
    let window = sample();
    let bytes = graph_wire::encode(&window);

    let rows = 2;
    let segments = 3;
    let text: usize = window
        .commits
        .iter()
        .map(|c| c.summary.len() + c.author_name.len() + c.author_email.len())
        .sum();
    let columns = rows * (8 + 4 + 4 + 2 + 2 + 1 + 1 + 40) + 4 + (rows * 3 + 1) * 4;
    assert_eq!(
        bytes.len(),
        32 + columns + segments * (2 + 2 + 1 + 1) + text
    );
}

#[test]
fn an_empty_window_is_a_header() {
    let window = GraphWindow {
        start: 0,
        total: 0,
        complete: true,
        commits: Vec::new(),
        rows: Vec::new(),
    };
    // The segment and text offset columns still end with their closing offset.
    assert_eq!(graph_wire::encode(&window).len(), 32 + 4 + 4);
}
