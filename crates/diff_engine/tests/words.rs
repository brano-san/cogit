// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use diff_engine::{DiffOptions, DiffRow, FileDiff, diff_text, inline_spans};

fn rows(diff: &FileDiff) -> &[DiffRow] {
    match diff {
        FileDiff::Text { hunks, .. } => &hunks[0].rows,
        other => panic!("expected a text diff, got {other:?}"),
    }
}

fn spans(rows: &[DiffRow], want_delete: bool) -> Vec<(u32, u32)> {
    rows.iter()
        .find_map(|row| match row {
            DiffRow::Delete { inline, .. } if want_delete => Some(inline.clone()),
            DiffRow::Insert { inline, .. } if !want_delete => Some(inline.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn slice_utf16(text: &str, span: (u32, u32)) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    String::from_utf16(&units[span.0 as usize..span.1 as usize]).unwrap()
}

#[test]
fn only_the_changed_word_is_marked() {
    let (old, new) = inline_spans("the quick brown fox", "the slow brown fox");

    assert_eq!(old.len(), 1);
    assert_eq!(new.len(), 1);
    assert_eq!(slice_utf16("the quick brown fox", old[0]), "quick");
    assert_eq!(slice_utf16("the slow brown fox", new[0]), "slow");
}

#[test]
fn identical_lines_have_no_spans() {
    let (old, new) = inline_spans("same text", "same text");

    assert!(old.is_empty());
    assert!(new.is_empty());
}

#[test]
fn offsets_are_utf16_units_so_javascript_can_slice_directly() {
    let before = "привет 🙂 мир";
    let after = "привет 🙂 世界";

    let (old, new) = inline_spans(before, after);

    assert_eq!(old, [(10, 13)], "🙂 is a surrogate pair: two UTF-16 units");
    assert_eq!(new, [(10, 12)]);
    assert_eq!(slice_utf16(before, old[0]), "мир");
    assert_eq!(slice_utf16(after, new[0]), "世界");
}

#[test]
fn an_emoji_replaced_by_an_emoji_keeps_its_boundaries() {
    let (old, new) = inline_spans("status 🙂 done", "status 🎉 done");

    assert_eq!(slice_utf16("status 🙂 done", old[0]), "🙂");
    assert_eq!(slice_utf16("status 🎉 done", new[0]), "🎉");
}

#[test]
fn neighbouring_changed_words_merge_into_one_span() {
    let (old, _) = inline_spans("a one two b", "a x y b");

    assert_eq!(old.len(), 1, "two adjacent words are one highlight");
    assert_eq!(slice_utf16("a one two b", old[0]), "one two");
}

#[test]
fn a_replaced_line_carries_spans_on_both_sides() {
    let diff = diff_text(
        "keep\nthe quick fox\n",
        "keep\nthe slow fox\n",
        &DiffOptions::default(),
    );
    let rows = rows(&diff);

    assert_eq!(spans(rows, true).len(), 1);
    assert_eq!(spans(rows, false).len(), 1);
}

#[test]
fn a_pure_insertion_gets_no_spans_to_compare_against() {
    let diff = diff_text("a\n", "a\nb\n", &DiffOptions::default());

    assert!(spans(rows(&diff), false).is_empty());
}

#[test]
fn word_diff_can_be_switched_off() {
    let options = DiffOptions {
        word_diff: false,
        ..DiffOptions::default()
    };

    let diff = diff_text("the quick fox\n", "the slow fox\n", &options);

    assert!(spans(rows(&diff), true).is_empty());
    assert!(spans(rows(&diff), false).is_empty());
}

#[test]
fn a_wildly_lopsided_block_disables_word_diff() {
    let old = "one line\n";
    let new: String = std::iter::once("one line changed\n".to_owned())
        .chain((0..200).map(|i| format!("added {i}\n")))
        .collect();

    let diff = diff_text(old, &new, &DiffOptions::default());

    assert!(
        spans(rows(&diff), true).is_empty(),
        "comparing 1 deleted line against 201 inserted ones is noise, not a word diff"
    );
}
