use crate::{BinaryCause, DiffSide};

/// A control character no text file holds. Tab, line feed, vertical tab, form feed,
/// carriage return and escape (terminal colours in a log) are text; so is DEL.
fn invalid(byte: u8) -> bool {
    matches!(byte, 0x00..=0x08 | 0x0E..=0x1A | 0x1C..=0x1F)
}

/// The first invalid character of `data`: its code, line and position in the line, both
/// from 1. The whole side is read, as SmartGit reads it, not git's first 8000 bytes: a text
/// limit of a million bytes keeps that cheap (R-531).
#[must_use]
pub fn invalid_character(data: &[u8]) -> Option<(u8, u32, u32)> {
    let at = data.iter().position(|&byte| invalid(byte))?;
    let before = &data[..at];
    let line_start = before
        .iter()
        .rposition(|&byte| byte == b'\n')
        .map_or(0, |nl| nl + 1);
    let line = before.iter().filter(|&&byte| byte == b'\n').count() + 1;
    // Characters, not bytes: a UTF-8 continuation byte adds nothing to the position.
    let position = before[line_start..]
        .iter()
        .filter(|&&byte| byte & 0xC0 != 0x80)
        .count()
        + 1;
    Some((
        data[at],
        u32::try_from(line).unwrap_or(u32::MAX),
        u32::try_from(position).unwrap_or(u32::MAX),
    ))
}

/// Why the two sides are not text, the old side asked first.
#[must_use]
pub fn cause(old: &[u8], new: &[u8]) -> Option<BinaryCause> {
    [(DiffSide::Old, old), (DiffSide::New, new)]
        .into_iter()
        .find_map(|(side, data)| {
            invalid_character(data).map(|(code, line, position)| BinaryCause::Character {
                code,
                line,
                position,
                side,
            })
        })
}
