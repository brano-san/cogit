//! A working-tree file opened for editing in the diff view: the text the editor shows and
//! the shape of the bytes it came from, so a save writes the same encoding, BOM, line
//! endings and final newline back. Anything that cannot round-trip is refused, not
//! normalized: the editor is disabled with the reason.

use crate::{GitError, RepoHandle, Result};
use serde::Serialize;

/// Past this the editor is not offered; the diff itself stops at the same size.
pub const MAX_EDIT_BYTES: usize = 1_000_000;

/// How the bytes on disk were made; `text` is the same content with `\n` line breaks and
/// without the final one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Shape {
    pub encoding: TextEncoding,
    pub bom: bool,
    pub eol: Eol,
    pub final_newline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum TextEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum Eol {
    Lf,
    Crlf,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum EditableFile {
    Text {
        text: String,
        shape: Shape,
        /// Blob id of the bytes read: a save checks the file still has it.
        stamp: String,
    },
    /// Why the editor is not offered, in words for the panel.
    Refused { reason: String },
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SaveOutcome {
    Saved {
        stamp: String,
    },
    /// Someone else wrote the file since it was read: nothing was written.
    ChangedOnDisk,
}

/// The text and its shape, or why it cannot be edited without changing bytes it did not touch.
pub fn decode(bytes: &[u8]) -> std::result::Result<(String, Shape), String> {
    if bytes.len() > MAX_EDIT_BYTES {
        return Err(format!(
            "The file is larger than {} MB; edit it in your editor.",
            MAX_EDIT_BYTES / 1_000_000
        ));
    }
    let (decoded, encoding, bom) = if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        (utf8(rest)?, TextEncoding::Utf8, true)
    } else if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        (
            utf16(rest, u16::from_le_bytes)?,
            TextEncoding::Utf16Le,
            true,
        )
    } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        (
            utf16(rest, u16::from_be_bytes)?,
            TextEncoding::Utf16Be,
            true,
        )
    } else {
        (utf8(bytes)?, TextEncoding::Utf8, false)
    };
    if decoded.contains('\0') {
        return Err("A binary file cannot be edited here.".to_owned());
    }
    let crlf = decoded.matches("\r\n").count();
    let lf = decoded.matches('\n').count() - crlf;
    let lone_cr = decoded.matches('\r').count() - crlf;
    if lone_cr > 0 || (crlf > 0 && lf > 0) {
        return Err(
            "The file mixes line endings; saving here would change lines you did not edit."
                .to_owned(),
        );
    }
    let eol = if crlf > 0 { Eol::Crlf } else { Eol::Lf };
    let text = decoded.replace("\r\n", "\n");
    let final_newline = text.ends_with('\n');
    let text = text.strip_suffix('\n').unwrap_or(&text).to_owned();
    Ok((
        text,
        Shape {
            encoding,
            bom,
            eol,
            final_newline,
        },
    ))
}

/// The bytes `decode` read `text` from, with the same shape.
#[must_use]
pub fn encode(text: &str, shape: Shape) -> Vec<u8> {
    let mut body = text.to_owned();
    if shape.final_newline {
        body.push('\n');
    }
    if shape.eol == Eol::Crlf {
        body = body.replace('\n', "\r\n");
    }
    match shape.encoding {
        TextEncoding::Utf8 => {
            let mut out = if shape.bom {
                vec![0xEF, 0xBB, 0xBF]
            } else {
                Vec::new()
            };
            out.extend_from_slice(body.as_bytes());
            out
        }
        TextEncoding::Utf16Le | TextEncoding::Utf16Be => {
            let le = shape.encoding == TextEncoding::Utf16Le;
            let mut out = Vec::with_capacity(2 + body.len() * 2);
            if shape.bom {
                out.extend_from_slice(if le { &[0xFF, 0xFE] } else { &[0xFE, 0xFF] });
            }
            for unit in body.encode_utf16() {
                out.extend_from_slice(&if le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            out
        }
    }
}

fn utf8(bytes: &[u8]) -> std::result::Result<String, String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        "The file is not UTF-8 or UTF-16 with a BOM; edit it in your editor.".to_owned()
    })
}

fn utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> std::result::Result<String, String> {
    let (pairs, rest) = bytes.as_chunks::<2>();
    if !rest.is_empty() {
        return Err("The UTF-16 file has an odd number of bytes.".to_owned());
    }
    let units: Vec<u16> = pairs.iter().map(|pair| unit(*pair)).collect();
    String::from_utf16(&units).map_err(|_| "The UTF-16 file is not valid UTF-16.".to_owned())
}

impl RepoHandle {
    fn editable_path(&self, path: &str) -> Result<std::path::PathBuf> {
        if self.is_bare() {
            return Err(GitError::InvalidState(
                "a bare repository has no working tree".to_owned(),
            ));
        }
        let file = self.root().join(path);
        if crate::blobs::is_symlink(&file) {
            return Err(GitError::InvalidState(format!(
                "{path} is a symbolic link: edit the file it points at instead"
            )));
        }
        Ok(file)
    }

    fn stamp_of(&self, bytes: &[u8]) -> String {
        gix::objs::compute_hash(self.repo.object_hash(), gix::objs::Kind::Blob, bytes)
            .map(|id| id.to_string())
            .unwrap_or_default()
    }

    pub fn read_editable(&self, path: &str) -> Result<EditableFile> {
        let file = match self.editable_path(path) {
            Ok(file) => file,
            Err(GitError::InvalidState(reason)) => return Ok(EditableFile::Refused { reason }),
            Err(err) => return Err(err),
        };
        let bytes = std::fs::read(&file)?;
        Ok(match decode(&bytes) {
            Ok((text, shape)) => EditableFile::Text {
                text,
                shape,
                stamp: self.stamp_of(&bytes),
            },
            Err(reason) => EditableFile::Refused { reason },
        })
    }

    /// `force` writes over a file that changed since `stamp` was read ("Keep mine").
    pub fn save_editable(
        &self,
        path: &str,
        text: &str,
        shape: Shape,
        stamp: &str,
        force: bool,
    ) -> Result<SaveOutcome> {
        let file = self.editable_path(path)?;
        if !force {
            let now = std::fs::read(&file).map(|bytes| self.stamp_of(&bytes));
            if now.ok().as_deref() != Some(stamp) {
                return Ok(SaveOutcome::ChangedOnDisk);
            }
        }
        let bytes = encode(text, shape);
        std::fs::write(&file, &bytes)?;
        Ok(SaveOutcome::Saved {
            stamp: self.stamp_of(&bytes),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(bytes: &[u8]) {
        let (text, shape) = decode(bytes).unwrap();
        assert_eq!(encode(&text, shape), bytes, "{shape:?}");
    }

    #[test]
    fn every_supported_shape_round_trips() {
        round_trip(b"a\nb\n");
        round_trip(b"a\nb");
        round_trip(b"a\r\nb\r\n");
        round_trip(b"a\r\nb");
        round_trip(b"\xEF\xBB\xBFa\r\n");
        round_trip(b"");
        round_trip(&[0xFF, 0xFE, b'a', 0, b'\r', 0, b'\n', 0]);
        round_trip(&[0xFE, 0xFF, 0, b'a', 0, b'\n']);
    }

    #[test]
    fn the_editor_sees_lf_and_no_final_newline() {
        let (text, shape) = decode(b"\xEF\xBB\xBFone\r\ntwo\r\n").unwrap();
        assert_eq!(text, "one\ntwo");
        assert!(shape.bom && shape.final_newline && shape.eol == Eol::Crlf);
        assert_eq!(
            encode("one\nnew\ntwo", shape),
            b"\xEF\xBB\xBFone\r\nnew\r\ntwo\r\n"
        );
    }

    #[test]
    fn what_cannot_round_trip_is_refused() {
        assert!(decode(b"a\r\nb\n").is_err(), "mixed endings");
        assert!(decode(b"a\rb").is_err(), "a lone CR");
        assert!(decode(b"\xFF\xFFnot utf8").is_err());
        assert!(decode(b"a\0b").is_err(), "binary");
        assert!(decode(&vec![b'a'; MAX_EDIT_BYTES + 1]).is_err());
    }
}
