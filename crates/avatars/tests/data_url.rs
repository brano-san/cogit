#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The picture crosses IPC as a data URL: the webview is given no path and no file access.

use avatars::data_url;

#[test]
fn it_announces_itself_as_a_png() {
    assert!(data_url(b"x").starts_with("data:image/png;base64,"));
}

#[test]
fn three_bytes_become_four_characters() {
    assert_eq!(data_url(b"Man"), "data:image/png;base64,TWFu");
}

#[test]
fn a_remainder_of_two_bytes_is_padded_once() {
    assert_eq!(data_url(b"Ma"), "data:image/png;base64,TWE=");
}

#[test]
fn a_remainder_of_one_byte_is_padded_twice() {
    assert_eq!(data_url(b"M"), "data:image/png;base64,TQ==");
}

#[test]
fn the_high_bytes_use_the_last_two_characters_of_the_alphabet() {
    assert_eq!(data_url(&[0xfb, 0xff, 0xfe]), "data:image/png;base64,+//+");
}

#[test]
fn nothing_encodes_to_nothing() {
    assert_eq!(data_url(b""), "data:image/png;base64,");
}
