//! An address in, a Gravatar URL out: trimmed and lowercased first, or one author
//! becomes two, and with `d=404` so that a miss is a miss and not a generated picture.

use md5::{Digest, Md5};

pub fn email_hash(email: &str) -> String {
    let normalised = email.trim().to_lowercase();
    let digest = Md5::digest(normalised.as_bytes());
    digest
        .iter()
        .fold(String::with_capacity(32), |mut out, byte| {
            use std::fmt::Write as _;
            let _ = write!(out, "{byte:02x}");
            out
        })
}

pub fn gravatar_url(email: &str, size: u32) -> String {
    format!(
        "https://www.gravatar.com/avatar/{}?s={size}&d=404",
        email_hash(email)
    )
}

pub fn is_noreply(email: &str) -> bool {
    email
        .rsplit_once('@')
        .is_some_and(|(_, domain)| domain.to_lowercase().contains("noreply"))
}
