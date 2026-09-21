//! Base64 by hand: a whole crate for forty lines used once is not worth the supply chain.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const PREFIX: &str = "data:image/png;base64,";

pub fn data_url(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(PREFIX.len() + bytes.len().div_ceil(3) * 4);
    out.push_str(PREFIX);

    for chunk in bytes.chunks(3) {
        let (b0, b1, b2) = (
            u32::from(chunk[0]),
            chunk.get(1).map_or(0, |b| u32::from(*b)),
            chunk.get(2).map_or(0, |b| u32::from(*b)),
        );
        let triple = (b0 << 16) | (b1 << 8) | b2;
        for shift in [18, 12, 6, 0] {
            out.push(char::from(ALPHABET[((triple >> shift) & 0x3f) as usize]));
        }
        let padding = 3 - chunk.len();
        for _ in 0..padding {
            out.pop();
        }
        for _ in 0..padding {
            out.push('=');
        }
    }
    out
}
