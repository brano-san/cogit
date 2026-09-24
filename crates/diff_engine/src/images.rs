#[must_use]
pub fn image_mime(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if data.starts_with(b"\xff\xd8\xff") {
        return Some("image/jpeg");
    }
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    if is_bmp(data) {
        return Some("image/bmp");
    }
    if is_svg(data) {
        return Some("image/svg+xml");
    }
    None
}

/// "BM", the four reserved bytes zero, and a DIB header of a size Windows defines: a text
/// file that starts with "BM" is not a bitmap.
fn is_bmp(data: &[u8]) -> bool {
    const HEADERS: [u32; 6] = [12, 40, 52, 56, 108, 124];
    data.len() >= 26
        && data.starts_with(b"BM")
        && data[6..10] == [0, 0, 0, 0]
        && HEADERS.contains(&u32::from_le_bytes([
            data[14], data[15], data[16], data[17],
        ]))
}

/// A document whose first element is `<svg`, after an optional byte-order mark, the XML
/// declaration, comments and a doctype. `<svg` anywhere else is text about an SVG.
fn is_svg(data: &[u8]) -> bool {
    let head = &data[..data.len().min(1024)];
    let text = String::from_utf8_lossy(head);
    let mut rest = text.trim_start_matches('\u{feff}').trim_start();
    loop {
        let skipped = if rest.starts_with("<?") {
            rest.find("?>").map(|end| &rest[end + 2..])
        } else if rest.starts_with("<!--") {
            rest.find("-->").map(|end| &rest[end + 3..])
        } else if rest.starts_with("<!DOCTYPE") || rest.starts_with("<!doctype") {
            rest.find('>').map(|end| &rest[end + 1..])
        } else {
            break;
        };
        let Some(after) = skipped else { return false };
        rest = after.trim_start();
    }
    rest.starts_with("<svg")
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[must_use]
pub fn data_url(mime: &str, data: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64(data))
}

#[must_use]
pub fn base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let indices = [n >> 18, (n >> 12) & 63, (n >> 6) & 63, n & 63];

        for (position, index) in indices.iter().enumerate() {
            if position > chunk.len() {
                out.push('=');
            } else {
                out.push(char::from(ALPHABET[*index as usize]));
            }
        }
    }
    out
}
