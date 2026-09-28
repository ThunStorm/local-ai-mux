use regex::Regex;
use std::{io::Write, sync::OnceLock};
use tracing_subscriber::EnvFilter;

fn patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r"(?i)authorization\s*[:=]\s*(?:Bearer\s+)?[^\s,;]+",
            r"(?i)Bearer\s+[A-Za-z0-9._~-]+",
            r"lmx_[A-Za-z0-9]{20,}",
            r"(?i)(?:api[_-]?key|access_token|refresh_token|password|cookie)\s*[:=]\s*[^\s,;]+",
            r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b",
        ]
        .into_iter()
        .map(|pattern| Regex::new(pattern).expect("redaction pattern"))
        .collect()
    })
}

pub fn redact(message: &str) -> String {
    patterns().iter().fold(message.to_owned(), |text, pattern| {
        pattern.replace_all(&text, "[REDACTED]").into_owned()
    })
}

pub fn redact_diagnostic(message: &str) -> String {
    let redacted = redact(message);
    static PATH: OnceLock<Regex> = OnceLock::new();
    PATH.get_or_init(|| {
        Regex::new(r"(?:/Users/|/home/|[A-Za-z]:\\Users\\)[^\s,;]+").expect("path pattern")
    })
    .replace_all(&redacted, "[PATH]")
    .into_owned()
}

#[derive(Default)]
struct RedactingWriter(Vec<u8>);
impl Write for RedactingWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Drop for RedactingWriter {
    fn drop(&mut self) {
        let message = String::from_utf8_lossy(&self.0);
        let _ = std::io::stderr().write_all(redact(&message).as_bytes());
    }
}

pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(RedactingWriter::default)
        .try_init();
}

#[cfg(test)]
mod tests {
    #[test]
    fn masks_sensitive_fields_and_paths() {
        let sample = "Authorization: Bearer lmx_abcdefghijklmnopqrstuvwxyz api_key=secret access_token=another refresh_token=third cookie=session password=abc user@example.com /Users/alice/private.txt";
        let redacted = super::redact_diagnostic(sample);
        for secret in [
            "abcdefghijklmnopqrstuvwxyz",
            "secret",
            "another",
            "third",
            "session",
            "password=abc",
            "user@example.com",
            "/Users/alice",
        ] {
            assert!(!redacted.contains(secret), "{secret}");
        }
    }
}
