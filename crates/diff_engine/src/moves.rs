use crate::{DiffRow, FileDiff};
use std::collections::HashMap;

/// Git's own floor for `--color-moved`: shorter runs are coincidence, not a move.
pub const MIN_MOVED_LINES: usize = 3;

/// Marks deletions and additions that carry the same lines, so a function moved across a
/// file reads as one fact instead of two blocks of noise.
pub fn detect_moves(diff: &mut FileDiff) {
    let FileDiff::Text { hunks, .. } = diff else {
        return;
    };

    let mut deleted: Vec<(usize, usize, String)> = Vec::new();
    let mut inserted: Vec<(usize, usize, String)> = Vec::new();
    for (h, hunk) in hunks.iter().enumerate() {
        for (r, row) in hunk.rows.iter().enumerate() {
            match row {
                DiffRow::Delete { text, .. } => deleted.push((h, r, normalize(text))),
                DiffRow::Insert { text, .. } => inserted.push((h, r, normalize(text))),
                _ => {}
            }
        }
    }

    let added_at: HashMap<&str, Vec<usize>> =
        inserted
            .iter()
            .enumerate()
            .fold(HashMap::new(), |mut map, (index, (_, _, text))| {
                map.entry(text.as_str()).or_default().push(index);
                map
            });

    let mut moved_deletes: Vec<Option<u32>> = vec![None; deleted.len()];
    let mut moved_inserts: Vec<Option<u32>> = vec![None; inserted.len()];
    let mut next_id: u32 = 0;

    let mut start = 0;
    while start < deleted.len() {
        let Some(candidates) = added_at.get(deleted[start].2.as_str()) else {
            start += 1;
            continue;
        };

        let best = candidates
            .iter()
            .map(|&begin| (begin, run_length(&deleted[start..], &inserted[begin..])))
            .max_by_key(|&(_, length)| length);

        match best {
            Some((begin, length)) if length >= MIN_MOVED_LINES => {
                let id = next_id;
                next_id += 1;
                for offset in 0..length {
                    moved_deletes[start + offset] = Some(id);
                    moved_inserts[begin + offset] = Some(id);
                }
                start += length;
            }
            _ => start += 1,
        }
    }

    apply(hunks, &deleted, &moved_deletes, true);
    apply(hunks, &inserted, &moved_inserts, false);
}

fn run_length(deleted: &[(usize, usize, String)], inserted: &[(usize, usize, String)]) -> usize {
    deleted
        .iter()
        .zip(inserted)
        .take_while(|((_, _, a), (_, _, b))| a == b && !a.is_empty())
        .count()
}

fn apply(
    hunks: &mut [crate::Hunk],
    entries: &[(usize, usize, String)],
    flags: &[Option<u32>],
    deletes: bool,
) {
    for ((hunk, row, _), &id) in entries.iter().zip(flags) {
        let Some(id) = id else {
            continue;
        };
        let Some(target) = hunks.get_mut(*hunk).and_then(|h| h.rows.get_mut(*row)) else {
            continue;
        };
        match target {
            DiffRow::Delete { moved, move_id, .. } if deletes => {
                *moved = true;
                *move_id = Some(id);
            }
            DiffRow::Insert { moved, move_id, .. } if !deletes => {
                *moved = true;
                *move_id = Some(id);
            }
            _ => {}
        }
    }
}

/// Indentation is ignored, as Git does: moving code usually re-indents it.
fn normalize(text: &str) -> String {
    text.trim().to_owned()
}
