pub mod file_log;
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

/// Dialogue capture preserves ordinary text (including email/path), but never recognized credentials.
pub fn redact_credentials(message: &str) -> String {
    redact_credentials_snapshot(message).into_owned()
}

/// Borrow ordinary text so recording a stream does not copy the cumulative
/// answer for every redaction pattern that has no match.
pub fn redact_credentials_snapshot(message: &str) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    static CREDENTIALS: OnceLock<Vec<Regex>> = OnceLock::new();
    let patterns = CREDENTIALS.get_or_init(|| [
        r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]+",
        r"\blmx_[A-Za-z0-9_-]*",
        r"\bsk-[A-Za-z0-9._~+/=-]*",
        r#"(?i)\b(?:authorization|api[_-]?key|access[_-]?token|refresh[_-]?token|password|cookie)\b["']?\s*[:=]\s*(?:"[^"]*(?:"|$)|'[^']*(?:'|$)|[^\s,;]+)"#,
    ].into_iter().map(|pattern| Regex::new(pattern).expect("credential pattern")).collect())
    ;
    let mut text = Cow::Borrowed(message);
    for pattern in patterns {
        if let Cow::Owned(replaced) = pattern.replace_all(&text, "[REDACTED]") {
            text = Cow::Owned(replaced);
        }
    }
    text
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
        let safe = redact_credentials(&redact(&message));
        let _ = std::io::stderr().write_all(safe.as_bytes());
        file_log::append(safe.as_bytes());
    }
}

pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    init_with_filter(filter);
}
fn init_with_filter(filter: EnvFilter) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(false)
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

#[cfg(test)]
mod credential_capture_tests {
    #[test]
    fn preserves_normal_dialogue_but_redacts_complete_and_unfinished_credentials() {
        let text="user@example.com /Users/alice/notes 正常正文 sk-proj-abcdef lmx_short api_key=unsafe \"password\":\"two words\" Bearer opaque.jwt";
        let safe = super::redact_credentials(text);
        for secret in ["abcdef", "lmx_short", "unsafe", "two words", "opaque.jwt"] {
            assert!(!safe.contains(secret));
        }
        assert!(safe.contains("user@example.com /Users/alice/notes 正常正文"));
        assert!(
            !super::redact_credentials("{\"api_key\":\"partial value").contains("partial value")
        );
        assert_eq!(super::redact_credentials("sk-"), "[REDACTED]");
    }
}

#[cfg(test)]
mod file_integration_tests {
    #[test]
    fn subscriber_writes_redacted_events_to_the_configured_file() {
        let dir = tempfile::tempdir().unwrap();
        super::file_log::init(dir.path().to_path_buf(), Default::default()).unwrap();
        super::init_with_filter(tracing_subscriber::EnvFilter::new("info"));
        tracing::info!("diagnostic sentinel api_key=unsafe sk-proj-abcdef");
        let text = std::fs::read_to_string(dir.path().join("localmux.log")).unwrap();
        assert!(
            text.contains("diagnostic sentinel"),
            "subscriber produced no file event"
        );
        assert!(!text.contains("unsafe"));
        assert!(!text.contains("abcdef"));
    }
}
