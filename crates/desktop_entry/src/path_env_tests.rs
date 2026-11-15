use super::*;

const DIR: &str = r"C:\Users\me\AppData\Local\Cogit";

#[test]
fn adds_once_at_the_end() {
    assert_eq!(with_entry("", DIR).as_deref(), Some(DIR));
    assert_eq!(
        with_entry(r"%USERPROFILE%\bin", DIR),
        Some(format!(r"%USERPROFILE%\bin;{DIR}"))
    );
    assert_eq!(with_entry(r"C:\a;", DIR), Some(format!(r"C:\a;{DIR}")));
    assert_eq!(with_entry(&format!(r"C:\a;{DIR}"), DIR), None);
}

#[test]
fn an_entry_spelled_differently_is_the_same_folder() {
    let variants = [
        DIR.to_uppercase(),
        format!("{DIR}\\"),
        format!("\"{DIR}\""),
        format!(" {DIR} "),
    ];
    for variant in variants {
        assert_eq!(with_entry(&format!(r"C:\a;{variant}"), DIR), None);
        assert_eq!(
            without_entry(&format!(r"C:\a;{variant};C:\b"), DIR).as_deref(),
            Some(r"C:\a;C:\b")
        );
    }
}

#[test]
fn removal_keeps_every_other_entry_as_written() {
    let path = format!(r"C:\a;;%SystemRoot%\x;{DIR};C:\Cogit2;C:\b");
    assert_eq!(
        without_entry(&path, DIR).as_deref(),
        Some(r"C:\a;;%SystemRoot%\x;C:\Cogit2;C:\b")
    );
    assert_eq!(without_entry(r"C:\a;C:\b", DIR), None);
    assert_eq!(without_entry(DIR, DIR).as_deref(), Some(""));
}

// NSIS strings end at 1024 characters; the edit must not.
#[test]
fn a_path_longer_than_nsis_strings_survives_whole() {
    let long: Vec<String> = (0..200)
        .map(|i| format!(r"C:\tools\tool-{i:03}\bin"))
        .collect();
    let long = long.join(";");
    assert!(long.len() > 4096);
    let added = with_entry(&long, DIR).unwrap();
    assert!(added.starts_with(&long));
    assert_eq!(without_entry(&added, DIR), Some(long));
}
