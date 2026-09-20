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
    if data.starts_with(b"BM") {
        return Some("image/bmp");
    }
    if is_svg(data) {
        return Some("image/svg+xml");
    }
    None
}

fn is_svg(data: &[u8]) -> bool {
    let head = &data[..data.len().min(1024)];
    let text = String::from_utf8_lossy(head);
    text.contains("<svg")
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[must_use]
pub fn data_url(mime: &str, data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4 + mime.len() + 16);
    out.push_str("data:");
    out.push_str(mime);
    out.push_str(";base64,");

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
