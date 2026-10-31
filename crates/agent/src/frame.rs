use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::value::RawValue;

pub type Json = Box<RawValue>;
use std::io::{self, Read, Write};

pub const PROTOCOL: u32 = 4;

const LARGEST_FRAME: u32 = 256 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "camelCase")]
pub enum ToAgent {
    Hello {
        protocol: u32,
        version: String,
    },
    Call {
        id: u64,
        request: Value,
        #[serde(default)]
        stopped: bool,
    },
    Cancel {
        id: u64,
    },
    Answer {
        id: u64,
        answer: Value,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "frame", content = "body", rename_all = "camelCase")]
pub enum FromAgent {
    Welcome {
        protocol: u32,
        version: String,
    },
    Reply {
        id: u64,
        outcome: Result<Json, Json>,
    },
    Chunk {
        id: u64,
        data: Json,
    },
    Event {
        event: Value,
    },
    Ask {
        id: u64,
        question: Value,
    },
}

pub fn write_frame<W: Write + ?Sized>(output: &mut W, frame: &impl Serialize) -> io::Result<()> {
    let body = serde_json::to_vec(frame)?;
    let length = u32::try_from(body.len())
        .ok()
        .filter(|length| *length <= LARGEST_FRAME)
        .ok_or_else(|| invalid(format!("a frame of {} bytes is too large", body.len())))?;
    output.write_all(&length.to_le_bytes())?;
    output.write_all(&body)?;
    output.flush()
}

pub fn read_frame<R: Read, T: DeserializeOwned>(input: &mut R) -> io::Result<Option<T>> {
    let mut length = [0; 4];
    match input.read_exact(&mut length) {
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        read => read?,
    }
    let length = u32::from_le_bytes(length);
    if length > LARGEST_FRAME {
        return Err(invalid(format!("a frame of {length} bytes is too large")));
    }
    let mut body = vec![0; length as usize];
    input.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}

pub(crate) fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
