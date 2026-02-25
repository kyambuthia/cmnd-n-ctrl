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

#[cfg(test)]
mod tests {
    use super::{decode_stdio_frame, encode_stdio_frame};

    #[test]
    fn round_trip_frame() {
        let payload = r#"{\"jsonrpc\":\"2.0\"}"#;
        let encoded = encode_stdio_frame(payload);
        let decoded = decode_stdio_frame(&encoded).expect("frame should decode");
        assert_eq!(decoded, payload);
    }
}
