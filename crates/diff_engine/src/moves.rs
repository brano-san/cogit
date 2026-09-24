use crate::{DiffRow, FileDiff, FileDiffEntry, MoveScope};
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

        // An insertion already paired with an earlier deletion is the end of that move;
        // offered again, a second copy of the block pointed at it too and the first move
        // was left with nothing on the other side.
        let best = candidates
            .iter()
            .map(|&begin| {
                let free = moved_inserts[begin..]
                    .iter()
                    .take_while(|taken| taken.is_none())
                    .count();
                let run = run_length(&deleted[start..], &inserted[begin..]).min(free);
                (begin, run)
            })
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

    apply(hunks, &deleted, &moved_deletes, true, MoveScope::WithinFile);
    apply(
        hunks,
        &inserted,
        &moved_inserts,
        false,
        MoveScope::WithinFile,
    );
}

/// The second pass, over a whole batch: a block that left one file for another cannot be
/// seen from inside either of them.
///
/// It also renumbers what the per-file pass found. Each file numbers its own moves from
/// zero, so without this two unrelated moves in two files would share an identifier and
/// the UI would draw them as one pair.
pub fn link_moves_across_files(entries: &mut [FileDiffEntry]) {
    let mut next_id = renumber(entries);

    let mut deleted: Vec<Site> = Vec::new();
    let mut inserted: Vec<Site> = Vec::new();
    for (file, entry) in entries.iter().enumerate() {
        collect_unmatched(&entry.diff, file, &mut deleted, &mut inserted);
    }

    let added_at: HashMap<&str, Vec<usize>> =
        inserted
            .iter()
            .enumerate()
            .fold(HashMap::new(), |mut map, (index, site)| {
                map.entry(site.text.as_str()).or_default().push(index);
                map
            });

    let mut pairs: Vec<(Site, Site, u32)> = Vec::new();
    let mut taken = vec![false; inserted.len()];
    let mut start = 0;

    while start < deleted.len() {
        let Some(candidates) = added_at.get(deleted[start].text.as_str()) else {
            start += 1;
            continue;
        };

        let best = candidates
            .iter()
            .filter(|&&begin| !taken[begin] && inserted[begin].file != deleted[start].file)
            .map(|&begin| (begin, cross_run(&deleted, start, &inserted, begin, &taken)))
            .max_by_key(|&(_, length)| length);

        match best {
            Some((begin, length)) if length >= MIN_MOVED_LINES => {
                let id = next_id;
                next_id += 1;
                for offset in 0..length {
                    taken[begin + offset] = true;
                    pairs.push((
                        deleted[start + offset].clone(),
                        inserted[begin + offset].clone(),
                        id,
                    ));
                }
                start += length;
            }
            _ => start += 1,
        }
    }

    for (delete, insert, id) in pairs {
        mark(entries, &delete, id, true);
        mark(entries, &insert, id, false);
    }
}

/// One row's address inside the batch.
#[derive(Debug, Clone)]
struct Site {
    file: usize,
    hunk: usize,
    row: usize,
    text: String,
}

/// Gives every move in the batch a number of its own and returns the next free one.
fn renumber(entries: &mut [FileDiffEntry]) -> u32 {
    let mut next: u32 = 0;
    for entry in entries.iter_mut() {
        let FileDiff::Text { hunks, .. } = &mut entry.diff else {
            continue;
        };
        let mut mapping: HashMap<u32, u32> = HashMap::new();
        for hunk in hunks.iter_mut() {
            for row in &mut hunk.rows {
                let (DiffRow::Delete { move_id, .. } | DiffRow::Insert { move_id, .. }) = row
                else {
                    continue;
                };
                let Some(local) = *move_id else { continue };
                let global = *mapping.entry(local).or_insert_with(|| {
                    let id = next;
                    next += 1;
                    id
                });
                *move_id = Some(global);
            }
        }
    }
    next
}

fn collect_unmatched(
    diff: &FileDiff,
    file: usize,
    deleted: &mut Vec<Site>,
    inserted: &mut Vec<Site>,
) {
    let FileDiff::Text { hunks, .. } = diff else {
        return;
    };
    for (h, hunk) in hunks.iter().enumerate() {
        for (r, row) in hunk.rows.iter().enumerate() {
            let site = |text: &str| Site {
                file,
                hunk: h,
                row: r,
                text: normalize(text),
            };
            match row {
                DiffRow::Delete { text, move_id, .. } if move_id.is_none() => {
                    deleted.push(site(text));
                }
                DiffRow::Insert { text, move_id, .. } if move_id.is_none() => {
                    inserted.push(site(text));
                }
                _ => {}
            }
        }
    }
}

fn mark(entries: &mut [FileDiffEntry], site: &Site, id: u32, deletes: bool) {
    let Some(entry) = entries.get_mut(site.file) else {
        return;
    };
    let FileDiff::Text { hunks, .. } = &mut entry.diff else {
        return;
    };
    let Some(target) = hunks
        .get_mut(site.hunk)
        .and_then(|hunk| hunk.rows.get_mut(site.row))
    else {
        return;
    };
    set(target, id, deletes, MoveScope::AcrossFiles);
}

fn run_length(deleted: &[(usize, usize, String)], inserted: &[(usize, usize, String)]) -> usize {
    deleted
        .iter()
        .zip(inserted)
        .take_while(|((_, _, a), (_, _, b))| a == b && !a.is_empty())
        .count()
}

/// Like `run_length`, but a run may not cross a file boundary on either side, and may not
/// reuse an insertion that an earlier run already claimed.
fn cross_run(
    deleted: &[Site],
    start: usize,
    inserted: &[Site],
    begin: usize,
    taken: &[bool],
) -> usize {
    let from = deleted[start].file;
    let to = inserted[begin].file;
    let mut length = 0;

    while start + length < deleted.len() && begin + length < inserted.len() {
        let delete = &deleted[start + length];
        let insert = &inserted[begin + length];
        let same = delete.text == insert.text && !delete.text.is_empty();
        if !same || delete.file != from || insert.file != to || taken[begin + length] {
            break;
        }
        length += 1;
    }
    length
}

fn apply(
    hunks: &mut [crate::Hunk],
    entries: &[(usize, usize, String)],
    flags: &[Option<u32>],
    deletes: bool,
    scope: MoveScope,
) {
    for ((hunk, row, _), &id) in entries.iter().zip(flags) {
        let Some(id) = id else {
            continue;
        };
        let Some(target) = hunks.get_mut(*hunk).and_then(|h| h.rows.get_mut(*row)) else {
            continue;
        };
        set(target, id, deletes, scope);
    }
}

fn set(row: &mut DiffRow, id: u32, deletes: bool, scope: MoveScope) {
    match row {
        DiffRow::Delete {
            moved,
            move_id,
            move_scope,
            ..
        } if deletes => {
            *moved = true;
            *move_id = Some(id);
            *move_scope = Some(scope);
        }
        DiffRow::Insert {
            moved,
            move_id,
            move_scope,
            ..
        } if !deletes => {
            *moved = true;
            *move_id = Some(id);
            *move_scope = Some(scope);
        }
        _ => {}
    }
}

/// Indentation is ignored, as Git does: moving code usually re-indents it.
fn normalize(text: &str) -> String {
    text.trim().to_owned()
}
