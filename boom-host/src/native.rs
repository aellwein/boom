//! Native messaging framing.
//!
//! Chrome and Firefox send each message as a 4-byte little-endian length
//! followed by that many bytes of UTF-8 JSON. Replies use the same framing.

use std::io::{self, Read, Write};

use serde::Serialize;

/// Largest message the host accepts from the browser. Browsers cap
/// host-to-browser messages at 1 MB, so the same limit is used here.
pub const MAX_MESSAGE_SIZE: u32 = 1024 * 1024;

/// Reads one framed message. Returns `Ok(None)` when the browser closed stdin.
pub fn read_message<R: Read>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut length_bytes = [0u8; 4];
    match reader.read_exact(&mut length_bytes) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err),
    }

    let length = u32::from_le_bytes(length_bytes);
    if length > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message too large: {length} bytes"),
        ));
    }

    let mut body = vec![0u8; length as usize];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

/// Writes one framed message containing `value` as JSON.
pub fn write_message<W: Write, T: Serialize>(writer: &mut W, value: &T) -> io::Result<()> {
    let body =
        serde_json::to_vec(value).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    let length = u32::try_from(body.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "response too large"))?;
    if length > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("response too large: {length} bytes"),
        ));
    }
    writer.write_all(&length.to_le_bytes())?;
    writer.write_all(&body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn writes_length_prefixed_json() {
        let mut buffer = Vec::new();
        write_message(&mut buffer, &serde_json::json!({"ok": true})).unwrap();

        let length = u32::from_le_bytes(buffer[0..4].try_into().unwrap()) as usize;
        assert_eq!(length, buffer.len() - 4);
        assert_eq!(&buffer[4..], br#"{"ok":true}"#);
    }

    #[test]
    fn reads_message_back() {
        let mut buffer = Vec::new();
        write_message(&mut buffer, &serde_json::json!({"id": "1"})).unwrap();

        let mut reader = Cursor::new(buffer);
        let body = read_message(&mut reader).unwrap().unwrap();
        assert_eq!(body, br#"{"id":"1"}"#);
        assert!(read_message(&mut reader).unwrap().is_none());
    }

    #[test]
    fn rejects_oversized_message() {
        let mut buffer = (MAX_MESSAGE_SIZE + 1).to_le_bytes().to_vec();
        buffer.extend_from_slice(b"{}");
        let mut reader = Cursor::new(buffer);
        assert!(read_message(&mut reader).is_err());
    }
}
