#![allow(clippy::unwrap_used, clippy::expect_used)]

//! How an address becomes a Gravatar URL, and what is drawn when there is no picture.
//! Both are pure: nothing here touches the network or the disk (M14 T14.3, T14.4).

use avatars::{email_hash, fallback, gravatar_url, is_noreply};

#[test]
fn the_hash_is_the_documented_gravatar_vector() {
    // Gravatar's own example, which pins trim + lowercase + md5 in one assertion.
    assert_eq!(
        email_hash("MyEmailAddress@example.com "),
        "0bc83cb571cd1c50ba6f3e8a78ef1346"
    );
}

#[test]
fn case_and_spacing_do_not_split_one_author_in_two() {
    assert_eq!(
        email_hash("  Ada@Example.COM"),
        email_hash("ada@example.com")
    );
}

#[test]
fn the_url_refuses_the_stand_in_image() {
    // Without `d=404` Gravatar answers with a generated picture and every author looks
    // found, so a miss could never be cached as a miss.
    assert!(gravatar_url("ada@example.com", 64).contains("d=404"));
}

#[test]
fn the_url_asks_for_the_size_it_was_given() {
    assert!(gravatar_url("ada@example.com", 64).contains("s=64"));
    assert!(gravatar_url("ada@example.com", 128).contains("s=128"));
}

#[test]
fn the_url_carries_the_hash_and_not_the_address() {
    let url = gravatar_url("ada@example.com", 64);
    assert!(url.contains(&email_hash("ada@example.com")), "{url}");
    assert!(!url.contains("ada@example.com"), "{url}");
}

#[test]
fn github_noreply_addresses_are_recognised() {
    assert!(is_noreply("12345+octocat@users.noreply.github.com"));
    assert!(is_noreply("octocat@users.noreply.github.com"));
    assert!(!is_noreply("ada@example.com"));
}

#[test]
fn initials_come_from_the_name() {
    assert_eq!(fallback("Ada Lovelace", "ada@example.com").initials, "AL");
}

#[test]
fn a_single_word_name_gives_one_letter() {
    assert_eq!(fallback("octocat", "o@example.com").initials, "O");
}

#[test]
fn only_the_first_and_last_word_are_used() {
    assert_eq!(
        fallback("Ada Augusta King Lovelace", "ada@example.com").initials,
        "AL"
    );
}

#[test]
fn a_non_latin_name_keeps_its_own_letters() {
    assert_eq!(fallback("Иван Петров", "ivan@example.com").initials, "ИП");
}

#[test]
fn a_nameless_author_falls_back_to_the_address() {
    assert_eq!(fallback("   ", "ada@example.com").initials, "A");
}

#[test]
fn an_author_with_neither_still_gets_something_to_draw() {
    assert_eq!(fallback("", "").initials, "?");
}

#[test]
fn the_colour_is_the_same_for_the_same_author_every_run() {
    let first = fallback("Ada Lovelace", "ada@example.com").color;
    let second = fallback("Ada Lovelace", "  ADA@Example.com").color;
    assert_eq!(first, second);
}

#[test]
fn the_colour_is_a_hex_triplet() {
    let color = fallback("Ada Lovelace", "ada@example.com").color;
    assert_eq!(color.len(), 7, "{color}");
    assert!(color.starts_with('#'), "{color}");
    assert!(color[1..].chars().all(|c| c.is_ascii_hexdigit()), "{color}");
}

#[test]
fn two_authors_are_unlikely_to_share_a_colour() {
    let a = fallback("Ada Lovelace", "ada@example.com").color;
    let b = fallback("Grace Hopper", "grace@example.com").color;
    assert_ne!(a, b);
}
