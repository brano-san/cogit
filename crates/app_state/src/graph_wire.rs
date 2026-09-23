//! A graph window as columns of numbers and one blob of text, for an `ArrayBuffer` on the
//! other side (R-194). Little-endian; every column starts aligned to its element size
//! because the columns go from the widest element to the narrowest:
//!
//! | what | type | count |
//! |---|---|---|
//! | header: version, start, rows, total, complete, segments, text bytes, oid bytes | u32 | 8 |
//! | commit time, Unix seconds | f64 | rows |
//! | first segment of each row, then the end | u32 | rows + 1 |
//! | text offsets: summary, author name, author email of each row, then the end | u32 | 3 × rows + 1 |
//! | time zone offset, minutes | i32 | rows |
//! | node lane, row width | u16 | rows each |
//! | segment from, segment to | u16 | segments each |
//! | node colour, node flags: kind in bits 0–1, primary in bit 2 | u8 | rows each |
//! | segment colour, segment flags: span in bits 0–1, primary in bit 2, arrow in bit 3 | u8 | segments each |
//! | object ids, fixed width | ASCII | rows × oid bytes |
//! | text | UTF-8 | text bytes |

use crate::GraphWindow;
use graph_engine::{NodeKind, Span};

pub const WIRE_VERSION: u32 = 1;

#[must_use]
pub fn encode(window: &GraphWindow) -> Vec<u8> {
    let rows = &window.rows;
    let commits = &window.commits;
    let segments: usize = rows.iter().map(|row| row.segments.len()).sum();
    let oid_bytes = commits.first().map_or(0, |commit| commit.oid.len());
    let text: usize = commits
        .iter()
        .map(|c| c.summary.len() + c.author_name.len() + c.author_email.len())
        .sum();

    let mut out = Vec::with_capacity(
        32 + rows.len() * (8 + 4 + 12 + 4 + 4 + 2 + oid_bytes) + segments * 6 + text + 8,
    );
    for value in [
        WIRE_VERSION,
        window.start,
        count(rows.len()),
        window.total,
        u32::from(window.complete),
        count(segments),
        count(text),
        count(oid_bytes),
    ] {
        out.extend_from_slice(&value.to_le_bytes());
    }

    for commit in commits {
        // Seconds since 1970 are exact in an f64 for the next hundred million years.
        #[allow(clippy::cast_precision_loss)]
        out.extend_from_slice(&(commit.timestamp as f64).to_le_bytes());
    }
    let mut first = 0;
    for row in rows {
        out.extend_from_slice(&count(first).to_le_bytes());
        first += row.segments.len();
    }
    out.extend_from_slice(&count(first).to_le_bytes());
    let mut offset = 0;
    for commit in commits {
        for field in [&commit.summary, &commit.author_name, &commit.author_email] {
            out.extend_from_slice(&count(offset).to_le_bytes());
            offset += field.len();
        }
    }
    out.extend_from_slice(&count(offset).to_le_bytes());
    for commit in commits {
        out.extend_from_slice(&commit.tz_offset_minutes.to_le_bytes());
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
        let mut id = commit.oid.as_bytes().to_vec();
        id.resize(oid_bytes, b'0');
        out.extend_from_slice(&id);
    }
    for commit in commits {
        out.extend_from_slice(commit.summary.as_bytes());
        out.extend_from_slice(commit.author_name.as_bytes());
        out.extend_from_slice(commit.author_email.as_bytes());
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
