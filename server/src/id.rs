use rand::Rng;

const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// 12-char base62 id (~3.2e21 space).
pub fn new_id() -> String {
    let mut bytes = [0u8; 12];
    rand::rng().fill(&mut bytes);
    bytes
        .iter()
        .map(|b| ALPHABET[*b as usize % 62] as char)
        .collect()
}

/// 64 lowercase hex chars (32 bytes entropy) — upload ownership token.
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
}

impl std::fmt::Display for NameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "filename is empty"),
            Self::TooLong => write!(f, "filename exceeds 2048 chars"),
        }
    }
}

impl std::error::Error for NameError {}

/// Strip C0/C1 control chars, trim, cap at 2048 chars. Empty after cleaning -> Err.
pub fn sanitize_name(raw: &str) -> Result<String, NameError> {
    let cleaned: String = raw.chars().filter(|c| !c.is_control()).collect();
    let trimmed = cleaned.trim().to_string();
    if trimmed.is_empty() {
        return Err(NameError::Empty);
    }
    if trimmed.chars().count() > 2048 {
        return Err(NameError::TooLong);
    }
    Ok(trimmed)
}

/// Safe for a quoted Content-Disposition filename: replaces `"`, `\`, CR, LF.
pub fn header_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '"' | '\\' | '\r' | '\n' => '_',
            other => other,
        })
        .collect()
}

/// RFC 5987 `filename*` percent-encoding (UTF-8). Unreserved: A-Z a-z 0-9 - _ . ~
pub fn pct_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        let c = *b as char;
        if b.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            out.push(c);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_12_base62() {
        for _ in 0..50 {
            let id = new_id();
            assert_eq!(id.len(), 12);
            assert!(id.chars().all(|c| c.is_ascii_alphanumeric()));
        }
    }

    #[test]
    fn tokens_are_64_hex() {
        let t = new_token();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn sanitize_strips_control_and_trims() {
        assert_eq!(sanitize_name("  hi\r\n\x07there  ").unwrap(), "hithere");
        assert!(sanitize_name("   \t\n").is_err());
        assert!(sanitize_name(&"a".repeat(2049)).is_err());
        assert_eq!(sanitize_name(&"a".repeat(2048)).unwrap().len(), 2048);
    }

    #[test]
    fn header_filename_neutralizes_danger() {
        assert!(!header_filename("a\"b").contains('"'));
        assert_eq!(header_filename("x\r\ny"), "x__y");
    }

    #[test]
    fn pct_encode_encodes_non_ascii() {
        assert_eq!(pct_encode("café.pdf"), "caf%C3%A9.pdf");
    }
}
