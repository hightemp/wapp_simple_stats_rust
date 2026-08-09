pub(crate) fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

pub(crate) fn strip_url_query(value: &str) -> &str {
    value
        .split_once(['?', '#'])
        .map_or(value, |(safe_part, _)| safe_part)
}

pub(crate) fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_url_query_removes_sensitive_query_parameters() {
        assert_eq!(
            strip_url_query("https://example.com/page?token=secret"),
            "https://example.com/page"
        );
    }

    #[test]
    fn encode_path_segment_escapes_reserved_characters() {
        assert_eq!(encode_path_segment("docs & help"), "docs%20%26%20help");
    }
}
