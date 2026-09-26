// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! A file shown as a summary says why: too large, or binary by its attributes or by a
//! character text does not hold, in SmartGit's words (R-531).

use diff_engine::{
    BinaryCause, BlobSide, Content, DiffOptions, DiffSide, FileDiff, MAX_TEXT_BYTES, diff_bytes,
    diff_bytes_as, invalid_character,
};

fn side(size: u64) -> Option<BlobSide> {
    Some(BlobSide { size, id: None })
}

#[test]
fn a_control_character_makes_a_file_binary_and_says_where() {
    let diff = diff_bytes(
        b"hello world\n",
        b"hello\x02world\n",
        &DiffOptions::default(),
    );

    match diff {
        FileDiff::Binary { old, new, cause } => {
            assert_eq!((old, new), (side(12), side(12)));
            assert_eq!(
                cause,
                BinaryCause::Character {
                    code: 0x02,
                    line: 1,
                    position: 6,
                    side: DiffSide::New,
                }
            );
        }
        other => panic!("expected a binary summary, got {other:?}"),
    }
}

#[test]
fn the_line_and_position_count_from_the_start_of_that_line_in_characters() {
    assert_eq!(invalid_character(b"one\ntwo\nab\x01"), Some((0x01, 3, 3)));
    assert_eq!(
        invalid_character("один\nдва\x00".as_bytes()),
        Some((0x00, 2, 4)),
        "Cyrillic letters are two bytes and one position each"
    );
}

#[test]
fn tabs_form_feeds_escapes_and_crlf_are_text() {
    let text = b"a\tb\r\n\x0cpage\n\x1b[1mbold\x1b[0m\n\x0bvt\x7f\n";

    assert_eq!(invalid_character(text), None);
    assert!(matches!(
        diff_bytes(text, b"other\n", &DiffOptions::default()),
        FileDiff::Text { .. }
    ));
}

#[test]
fn a_nul_past_the_first_eight_thousand_bytes_is_found_too() {
    let mut late = vec![b'a'; 9000];
    late.push(0);

    assert!(matches!(
        diff_bytes(b"a\n", &late, &DiffOptions::default()),
        FileDiff::Binary {
            cause: BinaryCause::Character { code: 0, .. },
            ..
        }
    ));
}

#[test]
fn the_attributes_call_a_file_binary_by_name() {
    let diff = diff_bytes_as(
        b"one\n",
        b"two\n",
        &DiffOptions::default(),
        &Content::Binary("-diff".to_owned()),
    );

    assert!(
        matches!(&diff, FileDiff::Binary { cause: BinaryCause::Attribute { name }, .. } if name == "-diff"),
        "{diff:?}"
    );
}

#[test]
fn the_attributes_call_a_file_text_whatever_it_holds() {
    let diff = diff_bytes_as(
        b"one\x02\n",
        b"two\x02\n",
        &DiffOptions::default(),
        &Content::Text,
    );

    assert!(matches!(diff, FileDiff::Text { .. }), "{diff:?}");
}

#[test]
fn equal_sides_are_unchanged_whatever_the_attributes_say() {
    let diff = diff_bytes_as(
        b"one\n",
        b"one\n",
        &DiffOptions::default(),
        &Content::Binary("binary".to_owned()),
    );

    assert!(matches!(diff, FileDiff::Unchanged), "{diff:?}");
}

#[test]
fn a_side_of_a_million_bytes_is_too_large_and_one_byte_less_is_not() {
    let limit = usize::try_from(MAX_TEXT_BYTES).unwrap();
    assert_eq!(limit, 1_000_000);
    let big = vec![b'a'; limit];

    let diff = diff_bytes(b"a\n", &big, &DiffOptions::default());
    assert!(
        matches!(&diff, FileDiff::TooLarge { old, new, limit: 1_000_000 }
            if *old == side(2) && *new == side(1_000_000)),
        "{diff:?}"
    );

    let fits = diff_bytes(b"a\n", &big[1..], &DiffOptions::default());
    assert!(matches!(fits, FileDiff::Text { .. }), "{fits:?}");
}
