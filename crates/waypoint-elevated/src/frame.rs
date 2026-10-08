// Length-prefixed frames over a byte stream: JSON control messages and raw data chunks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt;
use std::io::{self, Read, Write};

/// The most a frame may hold after its length: the kind byte and the body. A larger length is
/// refused before anything is allocated, so a hostile peer cannot make either side reserve memory.
pub const MAX_FRAME: usize = 4 * 1024 * 1024;

/// The most file content one data frame carries.
pub const MAX_CHUNK: usize = 256 * 1024;

const KIND_CONTROL: u8 = 1;
const KIND_DATA: u8 = 2;

/// The bytes of a data frame's request id.
const ID_LEN: usize = 8;

/// One unit of the stream. The wire form is a four-byte big-endian length, a one-byte kind and the
/// body; the length counts the kind byte and the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// A JSON message: a request, a response or an event.
    Control(Vec<u8>),
    /// File content for the request `id`, never encoded as text.
    Data { id: u64, bytes: Vec<u8> },
}

/// Why a frame could not be read or written. Every one but `Io` at a frame boundary means the
/// connection cannot be trusted any more and is closed.
#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    /// The frame is larger than `MAX_FRAME`, or a data chunk larger than `MAX_CHUNK`.
    TooLarge,
    /// The stream ended inside a frame.
    Truncated,
    /// A length of zero leaves no room for the kind.
    Empty,
    UnknownKind(u8),
    /// A data frame too short to hold its request id.
    BadData,
    /// A message that would not encode.
    Encode,
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::Io(error) => write!(f, "the stream failed: {error}"),
            FrameError::TooLarge => f.write_str("a frame is over the size limit"),
            FrameError::Truncated => f.write_str("the stream ended inside a frame"),
            FrameError::Empty => f.write_str("a frame has no kind"),
            FrameError::UnknownKind(kind) => write!(f, "a frame has the unknown kind {kind}"),
            FrameError::BadData => f.write_str("a data frame has no request id"),
            FrameError::Encode => f.write_str("a message could not be encoded"),
        }
    }
}

impl std::error::Error for FrameError {}

impl From<io::Error> for FrameError {
    fn from(error: io::Error) -> Self {
        FrameError::Io(error)
    }
}

/// Fills `buf`, telling a stream that ended before the first byte (`Ok(false)`) from one that
/// ended part way (`Truncated`).
fn fill(reader: &mut impl Read, buf: &mut [u8]) -> Result<bool, FrameError> {
    let mut done = 0;
    while done < buf.len() {
        match reader.read(&mut buf[done..]) {
            Ok(0) if done == 0 => return Ok(false),
            Ok(0) => return Err(FrameError::Truncated),
            Ok(n) => done += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(true)
}

/// Reads the next frame. `Ok(None)` is the end of the stream between frames.
pub fn read_frame(reader: &mut impl Read) -> Result<Option<Frame>, FrameError> {
    let mut header = [0u8; 4];
    if !fill(reader, &mut header)? {
        return Ok(None);
    }
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 {
        return Err(FrameError::Empty);
    }
    if length > MAX_FRAME {
        return Err(FrameError::TooLarge);
    }
    let mut kind = [0u8; 1];
    if !fill(reader, &mut kind)? {
        return Err(FrameError::Truncated);
    }
    let mut body = vec![0u8; length - 1];
    if !fill(reader, &mut body)? {
        return Err(FrameError::Truncated);
    }
    match kind[0] {
        KIND_CONTROL => Ok(Some(Frame::Control(body))),
        KIND_DATA => {
            if body.len() < ID_LEN {
                return Err(FrameError::BadData);
            }
            if body.len() - ID_LEN > MAX_CHUNK {
                return Err(FrameError::TooLarge);
            }
            let mut id = [0u8; ID_LEN];
            id.copy_from_slice(&body[..ID_LEN]);
            body.drain(..ID_LEN);
            Ok(Some(Frame::Data {
                id: u64::from_be_bytes(id),
                bytes: body,
            }))
        }
        other => Err(FrameError::UnknownKind(other)),
    }
}

/// Writes one frame and flushes. A frame over the limits is refused before anything is written.
pub fn write_frame(writer: &mut impl Write, frame: &Frame) -> Result<(), FrameError> {
    let (kind, id, payload): (u8, Option<u64>, &[u8]) = match frame {
        Frame::Control(body) => (KIND_CONTROL, None, body),
        Frame::Data { id, bytes } => {
            if bytes.len() > MAX_CHUNK {
                return Err(FrameError::TooLarge);
            }
            (KIND_DATA, Some(*id), bytes)
        }
    };
    let length = 1 + id.map_or(0, |_| ID_LEN) + payload.len();
    if length > MAX_FRAME {
        return Err(FrameError::TooLarge);
    }
    // One buffer and one write, so two threads sharing a writer under a lock never interleave and
    // a short write is a short frame, not a short header.
    let mut out = Vec::with_capacity(4 + length);
    out.extend_from_slice(&(length as u32).to_be_bytes());
    out.push(kind);
    if let Some(id) = id {
        out.extend_from_slice(&id.to_be_bytes());
    }
    out.extend_from_slice(payload);
    writer.write_all(&out)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn encoded(frame: &Frame) -> Vec<u8> {
        let mut out = Vec::new();
        write_frame(&mut out, frame).unwrap();
        out
    }

    fn read(bytes: &[u8]) -> Result<Option<Frame>, FrameError> {
        read_frame(&mut Cursor::new(bytes))
    }

    #[test]
    fn a_control_frame_round_trips() {
        let frame = Frame::Control(br#"{"a":1}"#.to_vec());
        let bytes = encoded(&frame);
        assert_eq!(&bytes[..5], &[0, 0, 0, 8, 1]);
        assert_eq!(read(&bytes).unwrap(), Some(frame));
    }

    #[test]
    fn a_data_frame_carries_raw_bytes_and_its_id() {
        let frame = Frame::Data {
            id: 0x0102_0304_0506_0708,
            bytes: vec![0, 255, 10, 13, 0],
        };
        let bytes = encoded(&frame);
        assert_eq!(bytes[4], 2);
        assert_eq!(&bytes[5..13], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&bytes[13..], &[0, 255, 10, 13, 0]);
        assert_eq!(read(&bytes).unwrap(), Some(frame));
    }

    #[test]
    fn frames_follow_one_another() {
        let mut bytes = encoded(&Frame::Control(b"{}".to_vec()));
        bytes.extend(encoded(&Frame::Data {
            id: 7,
            bytes: vec![1, 2, 3],
        }));
        let mut cursor = Cursor::new(bytes);
        assert!(matches!(
            read_frame(&mut cursor).unwrap(),
            Some(Frame::Control(_))
        ));
        assert!(matches!(
            read_frame(&mut cursor).unwrap(),
            Some(Frame::Data { id: 7, .. })
        ));
        assert!(read_frame(&mut cursor).unwrap().is_none());
    }

    #[test]
    fn the_end_between_frames_is_not_an_error() {
        assert!(read(&[]).unwrap().is_none());
    }

    #[test]
    fn an_empty_body_is_allowed() {
        let frame = Frame::Data {
            id: 1,
            bytes: Vec::new(),
        };
        assert_eq!(read(&encoded(&frame)).unwrap(), Some(frame));
    }

    #[test]
    fn a_length_over_the_cap_is_refused_before_the_body_is_read() {
        let mut bytes = ((MAX_FRAME + 1) as u32).to_be_bytes().to_vec();
        bytes.push(1);
        assert!(matches!(read(&bytes), Err(FrameError::TooLarge)));
        assert!(matches!(
            read(&u32::MAX.to_be_bytes()),
            Err(FrameError::TooLarge)
        ));
    }

    #[test]
    fn a_frame_at_the_cap_is_allowed() {
        let frame = Frame::Control(vec![b' '; MAX_FRAME - 1]);
        assert_eq!(read(&encoded(&frame)).unwrap(), Some(frame));
    }

    #[test]
    fn writing_a_frame_over_the_cap_writes_nothing() {
        let mut out = Vec::new();
        let result = write_frame(&mut out, &Frame::Control(vec![0; MAX_FRAME]));
        assert!(matches!(result, Err(FrameError::TooLarge)));
        assert!(out.is_empty());
        let result = write_frame(
            &mut out,
            &Frame::Data {
                id: 1,
                bytes: vec![0; MAX_CHUNK + 1],
            },
        );
        assert!(matches!(result, Err(FrameError::TooLarge)));
        assert!(out.is_empty());
    }

    #[test]
    fn a_data_chunk_over_the_chunk_cap_is_refused_on_reading() {
        let length = 1 + ID_LEN + MAX_CHUNK + 1;
        let mut bytes = (length as u32).to_be_bytes().to_vec();
        bytes.push(2);
        bytes.extend(vec![0; length - 1]);
        assert!(matches!(read(&bytes), Err(FrameError::TooLarge)));
    }

    #[test]
    fn a_truncated_header_or_body_is_an_error() {
        assert!(matches!(read(&[0, 0]), Err(FrameError::Truncated)));
        assert!(matches!(
            read(&[0, 0, 0, 9, 1, b'{']),
            Err(FrameError::Truncated)
        ));
        assert!(matches!(read(&[0, 0, 0, 5]), Err(FrameError::Truncated)));
    }

    #[test]
    fn a_zero_length_and_an_unknown_kind_are_errors() {
        assert!(matches!(read(&[0, 0, 0, 0]), Err(FrameError::Empty)));
        assert!(matches!(
            read(&[0, 0, 0, 2, 9, 0]),
            Err(FrameError::UnknownKind(9))
        ));
    }

    #[test]
    fn a_data_frame_too_short_for_its_id_is_an_error() {
        assert!(matches!(
            read(&[0, 0, 0, 3, 2, 0, 0]),
            Err(FrameError::BadData)
        ));
    }

    #[test]
    fn garbage_never_panics() {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..2000 {
            let mut bytes = Vec::new();
            for _ in 0..(state % 40) {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                bytes.push((state >> 33) as u8);
            }
            state = state.wrapping_add(1);
            let _ = read(&bytes);
        }
    }

    #[test]
    fn a_stream_that_fails_is_an_io_error() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::BrokenPipe))
            }
        }
        assert!(matches!(read_frame(&mut Broken), Err(FrameError::Io(_))));
    }
}
