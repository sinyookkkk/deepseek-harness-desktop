//! Sidecar stderr logging. Token-bearing values are redacted; stdout is never persisted.
//!
//! shortcut: the log appends without rotation. Add size-based rotation if the
//! file becomes large enough to matter for disk use.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// Appends redacted sidecar stderr lines to one log file.
#[derive(Debug, Clone)]
pub struct SidecarLog {
    path: PathBuf,
}

impl SidecarLog {
    /// Ensure the log directory exists.
    ///
    /// @param path - log file path.
    pub fn new(path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        Ok(Self { path })
    }

    /// Append one stderr line after redaction.
    ///
    /// A failed append never stops sidecar supervision; the in-memory stderr
    /// tail still reaches the error dialog.
    ///
    /// @param line - raw stderr line.
    pub fn append(&self, line: &str) {
        let redacted = redact(line);
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&self.path) {
            if let Err(error) = writeln!(file, "{redacted}") {
                eprintln!("dsh desktop: cannot write {}: {error}", self.path.display());
            }
        }
    }
}

/// Replace token parameter values, JSON token values, and Authorization values.
///
/// @param line - raw stderr text.
/// @returns the text with sensitive values replaced by `[redacted]`.
pub fn redact(line: &str) -> String {
    let line = redact_token_parameters(line);
    let line = redact_json_tokens(&line);
    redact_authorization(&line)
}

fn redact_token_parameters(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(index) = find_token_key(rest) {
        let (head, tail) = rest.split_at(index);
        output.push_str(head);
        let key = "token=";
        output.push_str(&tail[..key.len()]);
        let after = &tail[key.len()..];
        let end = after
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '&' | '"' | '\'' | '>' | ')')
            })
            .unwrap_or(after.len());
        output.push_str("[redacted]");
        rest = &after[end..];
    }
    output.push_str(rest);
    output
}

fn redact_json_tokens(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let mut output = String::with_capacity(line.len());
    let mut rest = line;
    let mut lower_rest = lower.as_str();
    loop {
        let Some(index) = lower_rest.find("\"token\"") else {
            output.push_str(rest);
            break;
        };
        let after_key = index + "\"token\"".len();
        let mut cursor = skip_ascii_whitespace(rest, after_key);
        if !rest[cursor..].starts_with(':') {
            output.push_str(&rest[..after_key]);
            rest = &rest[after_key..];
            lower_rest = &lower_rest[after_key..];
            continue;
        }
        cursor = skip_ascii_whitespace(rest, cursor + 1);
        if !rest[cursor..].starts_with('"') {
            output.push_str(&rest[..after_key]);
            rest = &rest[after_key..];
            lower_rest = &lower_rest[after_key..];
            continue;
        }
        let value_start = cursor + 1;
        let Some(end) = rest[value_start..].find('"') else {
            output.push_str(&rest[..value_start]);
            output.push_str("[redacted]");
            break;
        };
        let value_end = value_start + end;
        output.push_str(&rest[..value_start]);
        output.push_str("[redacted]");
        rest = &rest[value_end..];
        lower_rest = &lower_rest[value_end..];
    }
    output
}

fn redact_authorization(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let Some(index) = lower.find("authorization") else {
        return line.to_owned();
    };
    let after_key = index + "authorization".len();
    let cursor = skip_ascii_whitespace(line, after_key);
    if !line[cursor..].starts_with(':') {
        return line.to_owned();
    }
    let value_start = skip_ascii_whitespace(line, cursor + 1);
    if value_start >= line.len() {
        return line.to_owned();
    }
    format!("{}[redacted]", &line[..value_start])
}

fn skip_ascii_whitespace(line: &str, start: usize) -> usize {
    let mut cursor = start;
    for character in line[start..].chars() {
        if !character.is_ascii_whitespace() {
            break;
        }
        cursor += character.len_utf8();
    }
    cursor
}

/// Locate the next `token=` whose key starts at a delimiter boundary.
fn find_token_key(text: &str) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find("token=") {
        let index = from + found;
        let boundary = text[..index]
            .chars()
            .next_back()
            .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '_');
        if boundary {
            return Some(index);
        }
        from = index + "token=".len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let index = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("dsh-desktop-log-{}-{index}", std::process::id()));
            fs::create_dir_all(&path).expect("create temp dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn redacts_token_values() {
        assert_eq!(
            redact("GET /?token=s3cret&x=1 HTTP/1.1"),
            "GET /?token=[redacted]&x=1 HTTP/1.1"
        );
        assert_eq!(
            redact("authorization: bearer abc token=abc\r"),
            "authorization: [redacted]"
        );
        assert_eq!(redact("no secret here"), "no secret here");
    }

    #[test]
    fn redacts_json_tokens_and_authorization_headers() {
        assert_eq!(
            redact(r#"{"token": "abc123", "x": 1}"#),
            r#"{"token": "[redacted]", "x": 1}"#
        );
        assert_eq!(redact(r#"{"token":"abc123"}"#), r#"{"token":"[redacted]"}"#);
        assert_eq!(
            redact("Authorization: Bearer abc123"),
            "Authorization: [redacted]"
        );
    }

    #[test]
    fn token_line_is_redacted_in_the_log() {
        let dir = TempDir::new();
        let log = SidecarLog::new(dir.path().join("sidecar.log")).expect("create log");
        log.append("GET /?token=s3cret HTTP/1.1");
        let contents = fs::read_to_string(&log.path).expect("read log");
        assert!(contents.contains("token=[redacted]"), "log was {contents:?}");
        assert!(!contents.contains("s3cret"), "log was {contents:?}");
    }

    #[test]
    fn stdout_handshake_line_is_not_persisted() {
        let dir = TempDir::new();
        let log = SidecarLog::new(dir.path().join("sidecar.log")).expect("create log");
        log.append("sidecar started");
        let line = "dsh web: http://127.0.0.1:43123/?token=s3cret";
        assert!(crate::sidecar::parse_entry_url(line).is_some());
        let contents = fs::read_to_string(&log.path).expect("read log");
        assert!(!contents.contains("43123"), "stdout leaked into the log: {contents:?}");
        assert!(!contents.contains("s3cret"), "stdout leaked into the log: {contents:?}");
    }
}
