//! Drawn when there is no picture, and while there is not one yet: local by contract,
//! because a row must never wait on the network to look finished (M14 T14.4).

use crate::identity::email_hash;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Fallback {
    pub initials: String,
    pub color: String,
}

pub fn fallback(name: &str, email: &str) -> Fallback {
    Fallback {
        initials: initials(name, email),
        color: color(email),
    }
}

fn initials(name: &str, email: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let letters: Vec<char> = match words.as_slice() {
        [] => email.trim().chars().take(1).collect(),
        [only] => only.chars().take(1).collect(),
        [first, .., last] => first.chars().take(1).chain(last.chars().take(1)).collect(),
    };

    let text: String = letters.iter().flat_map(|c| c.to_uppercase()).collect();
    if text.is_empty() {
        "?".to_string()
    } else {
        text
    }
}

/// Hue from the hash, saturation and lightness fixed: a random triple is as likely to be
/// unreadable grey as anything else, and these sit under white initials.
fn color(email: &str) -> String {
    let hash = email_hash(email);
    let seed = u32::from_str_radix(&hash[..2], 16).unwrap_or(0);
    let (r, g, b) = hsl_to_rgb(f32::from(seed as u8) * 360.0 / 256.0, 0.45, 0.45);
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> (u8, u8, u8) {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue / 60.0;
    let second = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let (r, g, b) = match sector as u32 {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let base = lightness - chroma / 2.0;
    let byte = |v: f32| ((v + base) * 255.0).round().clamp(0.0, 255.0) as u8;
    (byte(r), byte(g), byte(b))
}
