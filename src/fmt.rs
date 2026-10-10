//! Small text helpers several crates need byte-for-byte alike.

/// `%XX` sequences decoded to bytes; anything else (a lone `%`, a non-hex
/// pair, a `+`) is kept literally.
///
/// Works on bytes throughout. Five crates each had a copy, and most sliced
/// the `&str` at `i + 1..i + 3`, which panics when that range ends inside a
/// multi-byte character (`%a` followed by `é`).
pub fn percent_decode_bytes(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = |j: usize| bytes.get(j).and_then(|b| (*b as char).to_digit(16));
            if let (Some(hi), Some(lo)) = (hex(i + 1), hex(i + 2)) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// [`percent_decode_bytes`] as text, with invalid UTF-8 replaced (U+FFFD).
pub fn percent_decode(s: &str) -> String {
    String::from_utf8_lossy(&percent_decode_bytes(s)).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_escapes_and_keeps_everything_else() {
        assert_eq!(percent_decode("my%20model.bin"), "my model.bin");
        assert_eq!(percent_decode("a+b%2Bc"), "a+b+c");
        assert_eq!(percent_decode("caf%C3%A9"), "café");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%4"), "%4");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[test]
    fn a_multibyte_character_after_a_percent_does_not_panic() {
        assert_eq!(percent_decode("%aé"), "%aé");
        assert_eq!(percent_decode("%é"), "%é");
    }

    #[test]
    fn invalid_utf8_is_replaced_in_text_and_kept_in_bytes() {
        assert_eq!(percent_decode_bytes("%FF"), vec![0xFF]);
        assert_eq!(percent_decode("%FF"), "\u{FFFD}");
    }
}
