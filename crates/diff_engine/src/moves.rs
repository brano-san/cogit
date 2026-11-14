use crate::{DiffRow, FileDiff, FileDiffEntry, Hunk, MoveScope};
use std::collections::HashMap;

/// Cogit's own floor: a line or two that match are coincidence more often than a move.
/// Git's default mode has none, only the letters-and-digits rule below.
pub const MIN_MOVED_LINES: usize = 3;

/// Git's `COLOR_MOVED_MIN_ALNUM_COUNT`: a block with fewer letters and digits is braces
/// and blank lines that happen to repeat.
pub const MIN_MOVED_ALNUM: usize = 20;

/// Marks deletions and additions that carry the same lines, so a function moved across a
/// file reads as one fact instead of two blocks of noise.
pub fn detect_moves(diff: &mut FileDiff) {
    let FileDiff::Text { hunks, .. } = diff else {
        return;
    };
    for (hunk, row, id, deletes) in within_file(hunks) {
        if let Some(target) = hunks.get_mut(hunk).and_then(|h| h.rows.get_mut(row)) {
            set(target, id, deletes, MoveScope::WithinFile);
        }
    }
}

/// `(hunk, row, move, deleted)` of each row that moved, read while the rows are borrowed.
fn within_file(hunks: &[Hunk]) -> Vec<(usize, usize, u32, bool)> {
    let mut deleted: Vec<(usize, usize, &str)> = Vec::new();
    let mut inserted: Vec<(usize, usize, &str)> = Vec::new();
    for (h, hunk) in hunks.iter().enumerate() {
        for (r, row) in hunk.rows.iter().enumerate() {
            match row {
                DiffRow::Delete { text, .. } => deleted.push((h, r, normalize(text))),
                DiffRow::Insert { text, .. } => inserted.push((h, r, normalize(text))),
                DiffRow::Context { .. } => {}
            }
        }
    }

    let added_at: HashMap<&str, Vec<usize>> =
        inserted
            .iter()
            .enumerate()
            .fold(HashMap::new(), |mut map, (index, &(_, _, text))| {
                map.entry(text).or_default().push(index);
                map
            });

    let mut moved_deletes: Vec<Option<u32>> = vec![None; deleted.len()];
    let mut moved_inserts: Vec<Option<u32>> = vec![None; inserted.len()];
    let mut next_id: u32 = 0;

    let mut start = 0;
    while start < deleted.len() {
        let Some(candidates) = added_at.get(deleted[start].2) else {
            start += 1;
            continue;
        };

        // An insertion already paired with an earlier deletion is the end of that move;
        // offered again, a second copy of the block pointed at it too and the first move
        // was left with nothing on the other side.
        let best = candidates
            .iter()
            .map(|&begin| {
                let run = run_length(&deleted[start..], &inserted[begin..]);
                let free = moved_inserts[begin..begin + run]
                    .iter()
                    .take_while(|taken| taken.is_none())
                    .count();
                (begin, free)
            })
            .max_by_key(|&(_, length)| length);

        match best {
            Some((begin, length))
                if length >= MIN_MOVED_LINES
                    && enough_text(deleted[start..start + length].iter().map(|row| row.2)) =>
            {
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

    let marks = |rows: &[(usize, usize, &str)], ids: &[Option<u32>], deletes: bool| {
        rows.iter()
            .zip(ids)
            .filter_map(move |(&(hunk, row, _), id)| id.map(|id| (hunk, row, id, deletes)))
            .collect::<Vec<_>>()
    };
    let mut moved = marks(&deleted, &moved_deletes, true);
    moved.extend(marks(&inserted, &moved_inserts, false));
    moved
}

/// The second pass, over a whole batch: a block that left one file for another cannot be
/// seen from inside either of them.
///
/// It also renumbers what the per-file pass found. Each file numbers its own moves from
/// zero, so without this two unrelated moves in two files would share an identifier and
/// the UI would draw them as one pair.
pub fn link_moves_across_files(entries: &mut [FileDiffEntry]) {
    let next_id = renumber(entries);
    for (delete, insert, id) in across_files(entries, next_id) {
        mark(entries, delete, id, true);
        mark(entries, insert, id, false);
    }
}

/// Each pair of rows a move across files joins, with its number, read while the rows are
/// borrowed.
fn across_files(entries: &[FileDiffEntry], mut next_id: u32) -> Vec<(Address, Address, u32)> {
    let mut deleted: Vec<Site<'_>> = Vec::new();
    let mut inserted: Vec<Site<'_>> = Vec::new();
    for (file, entry) in entries.iter().enumerate() {
        collect_unmatched(&entry.diff, file, &mut deleted, &mut inserted);
    }

    let added_at: HashMap<&str, Vec<usize>> =
        inserted
            .iter()
            .enumerate()
            .fold(HashMap::new(), |mut map, (index, site)| {
                map.entry(site.text).or_default().push(index);
                map
            });

    let mut pairs: Vec<(Address, Address, u32)> = Vec::new();
    let mut taken = vec![false; inserted.len()];
    let mut start = 0;

    while start < deleted.len() {
        let Some(candidates) = added_at.get(deleted[start].text) else {
            start += 1;
            continue;
        };

        let best = candidates
            .iter()
            .filter(|&&begin| !taken[begin] && inserted[begin].file != deleted[start].file)
            .map(|&begin| (begin, cross_run(&deleted, start, &inserted, begin, &taken)))
            .max_by_key(|&(_, length)| length);

        match best {
            Some((begin, length))
                if length >= MIN_MOVED_LINES
                    && enough_text(deleted[start..start + length].iter().map(|site| site.text)) =>
            {
                let id = next_id;
                next_id += 1;
                for offset in 0..length {
                    taken[begin + offset] = true;
                    pairs.push((
                        deleted[start + offset].address(),
                        inserted[begin + offset].address(),
                        id,
                    ));
                }
                start += length;
            }
            _ => start += 1,
        }
    }
    pairs
}

/// One row's address inside the batch: file, hunk, row.
type Address = (usize, usize, usize);

/// A row and its text, as moves compare it.
#[derive(Debug, Clone, Copy)]
struct Site<'a> {
    file: usize,
    hunk: usize,
    row: usize,
    text: &'a str,
}

impl Site<'_> {
    fn address(&self) -> Address {
        (self.file, self.hunk, self.row)
    }
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

fn collect_unmatched<'a>(
    diff: &'a FileDiff,
    file: usize,
    deleted: &mut Vec<Site<'a>>,
    inserted: &mut Vec<Site<'a>>,
) {
    let FileDiff::Text { hunks, .. } = diff else {
        return;
    };
    for (h, hunk) in hunks.iter().enumerate() {
        for (r, row) in hunk.rows.iter().enumerate() {
            let site = |text: &'a str| Site {
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

fn mark(entries: &mut [FileDiffEntry], (file, hunk, row): Address, id: u32, deletes: bool) {
    let Some(entry) = entries.get_mut(file) else {
        return;
    };
    let FileDiff::Text { hunks, .. } = &mut entry.diff else {
        return;
    };
    let Some(target) = hunks.get_mut(hunk).and_then(|hunk| hunk.rows.get_mut(row)) else {
        return;
    };
    set(target, id, deletes, MoveScope::AcrossFiles);
}

/// Equal rows in step on both sides. A run never spans a line that stayed: the rows of each
/// side must follow one another in one hunk, or three braces far apart pair up with three
/// together.
fn run_length(deleted: &[(usize, usize, &str)], inserted: &[(usize, usize, &str)]) -> usize {
    let mut length = 0;
    while let (Some(delete), Some(insert)) = (deleted.get(length), inserted.get(length)) {
        if delete.2 != insert.2 || delete.2.is_empty() {
            break;
        }
        if length > 0 {
            let (before, after) = (&deleted[length - 1], &inserted[length - 1]);
            if !follows((before.0, before.1), (delete.0, delete.1))
                || !follows((after.0, after.1), (insert.0, insert.1))
            {
                break;
            }
        }
        length += 1;
    }
    length
}

/// `(hunk, row)` of the row right after the other one.
fn follows(previous: (usize, usize), next: (usize, usize)) -> bool {
    previous.0 == next.0 && previous.1 + 1 == next.1
}

fn enough_text<'a>(rows: impl Iterator<Item = &'a str>) -> bool {
    rows.flat_map(|text| text.chars())
        .filter(|c| c.is_alphanumeric())
        .take(MIN_MOVED_ALNUM)
        .count()
        >= MIN_MOVED_ALNUM
}

/// Like `run_length`, but a run may not cross a file boundary on either side, and may not
/// reuse an insertion that an earlier run already claimed.
fn cross_run(
    deleted: &[Site<'_>],
    start: usize,
    inserted: &[Site<'_>],
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
        if length > 0 {
            let (before, after) = (&deleted[start + length - 1], &inserted[begin + length - 1]);
            if !follows((before.hunk, before.row), (delete.hunk, delete.row))
                || !follows((after.hunk, after.row), (insert.hunk, insert.row))
            {
                break;
            }
        }
        length += 1;
    }
    length
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
fn normalize(text: &str) -> &str {
    text.trim()
}
