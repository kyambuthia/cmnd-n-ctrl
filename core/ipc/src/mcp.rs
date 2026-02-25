use std::io::{self, BufRead, ErrorKind, Write};

pub fn encode_stdio_frame(payload: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", payload.len(), payload)
}

pub fn decode_stdio_frame(input: &str) -> Option<String> {
    let (header, body) = input.split_once("\r\n\r\n")?;
    let len_line = header
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("content-length:"))?;
    let length: usize = len_line.split(':').nth(1)?.trim().parse().ok()?;
    if body.len() < length {
        return None;
    }
    Some(body[..length].to_string())
}

pub fn read_stdio_frame_from<R: BufRead>(reader: &mut R) -> io::Result<Option<String>> {
    let mut content_length: Option<usize> = None;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            return Ok(None);
        }

        if line == "\r\n" || line == "\n" {
            break;
        }

        let line = line.trim_end_matches(&['\r', '\n'][..]);
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                let parsed = value.trim().parse::<usize>().map_err(|err| {
                    io::Error::new(ErrorKind::InvalidData, format!("invalid Content-Length: {err}"))
                })?;
                content_length = Some(parsed);
            }
        }
    }

    let length = content_length.ok_or_else(|| {
        io::Error::new(ErrorKind::InvalidData, "missing Content-Length header")
    })?;
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    String::from_utf8(body)
        .map(Some)
        .map_err(|err| io::Error::new(ErrorKind::InvalidData, format!("non-utf8 payload: {err}")))
}

pub fn write_stdio_frame_to<W: Write>(writer: &mut W, payload: &str) -> io::Result<()> {
    writer.write_all(encode_stdio_frame(payload).as_bytes())?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{decode_stdio_frame, encode_stdio_frame, read_stdio_frame_from, write_stdio_frame_to};

    #[test]
    fn round_trip_frame() {
        let payload = r#"{\"jsonrpc\":\"2.0\"}"#;
        let encoded = encode_stdio_frame(payload);
        let decoded = decode_stdio_frame(&encoded).expect("frame should decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn io_helpers_round_trip_frame() {
        let payload = r#"{"jsonrpc":"2.0","id":1}"#;
        let mut bytes = Vec::new();
        write_stdio_frame_to(&mut bytes, payload).expect("write frame");

        let mut cursor = Cursor::new(bytes);
        let decoded = read_stdio_frame_from(&mut cursor)
            .expect("read frame")
            .expect("payload exists");
        assert_eq!(decoded, payload);
    }
}
