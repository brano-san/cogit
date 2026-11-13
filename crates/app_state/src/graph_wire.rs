//! A graph window as columns of numbers and one blob of text, for an `ArrayBuffer` on the
//! other side (R-194). Little-endian; every column starts aligned to its element size
//! because the columns go from the widest element to the narrowest:
//!
//! | what | type | count |
//! |---|---|---|
//! | header: version, start, rows, total, complete, segments, text bytes, oid bytes | u32 | 8 |
//! | commit time, Unix seconds | f64 | rows |
//! | first segment of each row, then the end | u32 | rows + 1 |
//! | first long link of each row, then the end | u32 | rows + 1 |
//! | text offsets: summary, author name, author email of each row, then the end | u32 | 3 × rows + 1 |
//! | time zone offset, minutes | i32 | rows |
//! | node lane, row width | u16 | rows each |
//! | segment from, segment to | u16 | segments each |
//! | segment of each long link, counted within its row | u16 | links |
//! | node colour, node flags: kind in bits 0–1, primary in bit 2 | u8 | rows each |
//! | segment colour, segment flags: span in bits 0–1, primary in bit 2, arrow in bit 3 | u8 | segments each |
//! | object ids, fixed width | ASCII | rows × oid bytes |
//! | the far end of each long link | ASCII | links × oid bytes |
//! | text | UTF-8 | text bytes |

use crate::GraphWindow;
use git_engine::CommitRow;
use graph_engine::{GraphRow, NodeKind, Span};

pub const WIRE_VERSION: u32 = 2;

/// What the wire takes of a commit: a row a window carries, or a laid-out node with its
/// text, written straight from the cache without a `CommitRow` in between.
pub(crate) trait WireCommit {
    fn timestamp(&self) -> i64;
    fn tz_offset_minutes(&self) -> i32;
    /// Summary, author name, author email.
    fn texts(&self) -> [&str; 3];
    fn oid_len(&self) -> usize;
    /// The id in hex, `oid_bytes` long: padded with `0` when shorter.
    fn write_oid(&self, out: &mut Vec<u8>, oid_bytes: usize);
}

impl WireCommit for CommitRow {
    fn timestamp(&self) -> i64 {
        self.timestamp
    }

    fn tz_offset_minutes(&self) -> i32 {
        self.tz_offset_minutes
    }

    fn texts(&self) -> [&str; 3] {
        [&self.summary, &self.author_name, &self.author_email]
    }

    fn oid_len(&self) -> usize {
        self.oid.len()
    }

    fn write_oid(&self, out: &mut Vec<u8>, oid_bytes: usize) {
        write_padded(out, self.oid.as_bytes(), oid_bytes);
    }
}

pub(crate) fn write_padded(out: &mut Vec<u8>, hex: &[u8], oid_bytes: usize) {
    let hex = &hex[..hex.len().min(oid_bytes)];
    out.extend_from_slice(hex);
    out.resize(out.len() + (oid_bytes - hex.len()), b'0');
}

#[must_use]
pub fn encode(window: &GraphWindow) -> Vec<u8> {
    encode_rows(
        window.start,
        window.total,
        window.complete,
        &window.commits,
        &window.rows,
    )
}

pub(crate) fn encode_rows<C: WireCommit>(
    start: u32,
    total: u32,
    complete: bool,
    commits: &[C],
    rows: &[GraphRow],
) -> Vec<u8> {
    let segments: usize = rows.iter().map(|row| row.segments.len()).sum();
    let oid_bytes = commits.first().map_or(0, WireCommit::oid_len);
    let text: usize = commits
        .iter()
        .map(|c| c.texts().iter().map(|t| t.len()).sum::<usize>())
        .sum();

    let mut out = Vec::with_capacity(
        32 + rows.len() * (8 + 4 + 12 + 4 + 4 + 2 + oid_bytes) + segments * 6 + text + 8,
    );
    for value in [
        WIRE_VERSION,
        start,
        count(rows.len()),
        total,
        u32::from(complete),
        count(segments),
        count(text),
        count(oid_bytes),
    ] {
        out.extend_from_slice(&value.to_le_bytes());
    }

    for commit in commits {
        // Seconds since 1970 are exact in an f64 for the next hundred million years.
        #[allow(clippy::cast_precision_loss)]
        out.extend_from_slice(&(commit.timestamp() as f64).to_le_bytes());
    }
    let mut first = 0;
    for row in rows {
        out.extend_from_slice(&count(first).to_le_bytes());
        first += row.segments.len();
    }
    out.extend_from_slice(&count(first).to_le_bytes());
    let mut first_link = 0;
    for row in rows {
        out.extend_from_slice(&count(first_link).to_le_bytes());
        first_link += row.links.len();
    }
    out.extend_from_slice(&count(first_link).to_le_bytes());
    let mut offset = 0;
    for commit in commits {
        for field in commit.texts() {
            out.extend_from_slice(&count(offset).to_le_bytes());
            offset += field.len();
        }
    }
    out.extend_from_slice(&count(offset).to_le_bytes());
    for commit in commits {
        out.extend_from_slice(&commit.tz_offset_minutes().to_le_bytes());
    }

    for row in rows {
        out.extend_from_slice(&row.lane.to_le_bytes());
    }
    for row in rows {
        out.extend_from_slice(&row.width.to_le_bytes());
    }
    for segment in rows.iter().flat_map(|row| &row.segments) {
        out.extend_from_slice(&segment.from.to_le_bytes());
    }
    for segment in rows.iter().flat_map(|row| &row.segments) {
        out.extend_from_slice(&segment.to.to_le_bytes());
    }
    for link in rows.iter().flat_map(|row| &row.links) {
        out.extend_from_slice(&link.segment.to_le_bytes());
    }

    out.extend(rows.iter().map(|row| row.color));
    out.extend(
        rows.iter()
            .map(|row| kind_bits(row.kind) | u8::from(row.primary) << 2),
    );
    out.extend(rows.iter().flat_map(|row| &row.segments).map(|s| s.color));
    out.extend(
        rows.iter()
            .flat_map(|row| &row.segments)
            .map(|s| span_bits(s.span) | u8::from(s.primary) << 2 | u8::from(s.arrow) << 3),
    );

    // One repository hashes every object the same way, so every id is as long as the first.
    for commit in commits {
        commit.write_oid(&mut out, oid_bytes);
    }
    for link in rows.iter().flat_map(|row| &row.links) {
        write_padded(&mut out, link.oid.as_bytes(), oid_bytes);
    }
    for commit in commits {
        for field in commit.texts() {
            out.extend_from_slice(field.as_bytes());
        }
    }
    out
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

const fn kind_bits(kind: NodeKind) -> u8 {
    match kind {
        NodeKind::Normal => 0,
        NodeKind::Merge => 1,
        NodeKind::Root => 2,
        NodeKind::WorkingTree => 3,
    }
}

const fn span_bits(span: Span) -> u8 {
    match span {
        Span::Top => 0,
        Span::Bottom => 1,
        Span::Through => 2,
    }
}
