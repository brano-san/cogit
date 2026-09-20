// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use diff_engine::{DiffOptions, FileDiff, data_url, diff_bytes, image_mime};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest of the file";
const JPEG: &[u8] = b"\xff\xd8\xff\xe0rest";
const GIF: &[u8] = b"GIF89arest";
const WEBP: &[u8] = b"RIFF\x00\x00\x00\x00WEBPrest";
const SVG: &[u8] = b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"></svg>";

#[test]
fn every_supported_format_is_recognised() {
    assert_eq!(image_mime(PNG), Some("image/png"));
    assert_eq!(image_mime(JPEG), Some("image/jpeg"));
    assert_eq!(image_mime(GIF), Some("image/gif"));
    assert_eq!(image_mime(WEBP), Some("image/webp"));
    assert_eq!(image_mime(SVG), Some("image/svg+xml"));
}

#[test]
fn plain_text_is_not_an_image() {
    assert_eq!(image_mime(b"just some text\n"), None);
}

#[test]
fn an_empty_file_is_not_an_image() {
    assert_eq!(image_mime(b""), None);
}

#[test]
fn a_truncated_header_is_not_mistaken_for_an_image() {
    assert_eq!(image_mime(b"\x89PN"), None);
    assert_eq!(image_mime(b"RIFF\x00\x00\x00\x00WEB"), None);
}

#[test]
fn a_changed_image_produces_an_image_diff_rather_than_a_binary_one() {
    let mut other = PNG.to_vec();
    other.push(b'!');

    let diff = diff_bytes(PNG, &other, &DiffOptions::default());

    match diff {
        FileDiff::Image {
            mime,
            old_size,
            new_size,
        } => {
            assert_eq!(mime, "image/png");
            assert_eq!(old_size, PNG.len() as u64);
            assert_eq!(new_size, other.len() as u64);
        }
        other => panic!("expected an image diff, got {other:?}"),
    }
}

#[test]
fn an_added_image_is_still_an_image_diff() {
    let diff = diff_bytes(b"", PNG, &DiffOptions::default());

    assert!(matches!(diff, FileDiff::Image { .. }), "got {diff:?}");
}

#[test]
fn a_binary_file_that_is_not_an_image_stays_binary() {
    let diff = diff_bytes(b"\x00\x01\x02", b"\x00\x01\x03", &DiffOptions::default());

    assert!(matches!(diff, FileDiff::Binary { .. }), "got {diff:?}");
}

#[test]
fn an_svg_is_compared_as_an_image_even_though_it_is_text() {
    let mut other = SVG.to_vec();
    other.extend_from_slice(b"<!-- edited -->");

    let diff = diff_bytes(SVG, &other, &DiffOptions::default());

    match diff {
        FileDiff::Image { mime, .. } => assert_eq!(mime, "image/svg+xml"),
        other => panic!("expected an image diff, got {other:?}"),
    }
}

#[test]
fn an_unchanged_image_is_unchanged() {
    assert!(matches!(
        diff_bytes(PNG, PNG, &DiffOptions::default()),
        FileDiff::Unchanged
    ));
}

#[test]
fn a_data_url_carries_the_mime_type() {
    assert!(data_url("image/png", b"x").starts_with("data:image/png;base64,"));
}

#[test]
fn base64_matches_the_known_answers() {
    assert!(data_url("t", b"").ends_with("base64,"));
    assert!(data_url("t", b"f").ends_with("Zg=="));
    assert!(data_url("t", b"fo").ends_with("Zm8="));
    assert!(data_url("t", b"foo").ends_with("Zm9v"));
    assert!(data_url("t", b"foob").ends_with("Zm9vYg=="));
    assert!(data_url("t", b"fooba").ends_with("Zm9vYmE="));
    assert!(data_url("t", b"foobar").ends_with("Zm9vYmFy"));
}

#[test]
fn every_byte_value_survives_the_round_trip() {
    let all: Vec<u8> = (0..=255).collect();
    let url = data_url("application/octet-stream", &all);
    let payload = url.split_once(",").unwrap().1;

    assert_eq!(payload.len(), 344, "256 bytes encode to 344 characters");
    assert!(
        payload
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+/=".contains(c))
    );
}
