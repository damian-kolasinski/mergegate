use std::sync::LazyLock;

use regex::Regex;

/// A second defense for user-selected prose. Recorded telemetry itself
/// allowlists fields and never captures arbitrary messages or payloads.
pub fn sanitize(text: &str) -> String {
    static RULES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
        [
            r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?(?:-----END [A-Z ]*PRIVATE KEY-----|$)",
            r"(?i)\b(?:authorization|proxy-authorization|cookie|set-cookie)\s*:[^\r\n]*",
            r#"(?i)[\"']?\b(?:[a-z0-9_]*(?:token|password|passwd|secret|api[_-]?key|credential)[a-z0-9_]*)[\"']?\s*[:=]\s*(?:\"[^\"]*\"|'[^']*'|[^\s,;&]+)"#,
            r"(?i)\b(?:bearer|basic)\s+[a-z0-9._~+/=-]+",
            r"\b(?:gh[pousr]_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+|sk-[A-Za-z0-9_-]+|AKIA[A-Z0-9]{16})\b",
            r#"[a-zA-Z][a-zA-Z0-9+.-]*://[^\s<>"']+"#,
            r#"(?:/Users/|/home/|/private/|/tmp/|/var/|/opt/)[^\s"'<>]*"#,
            r#"[A-Za-z]:\\(?:[^\s"'<>]+)"#,
            r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}",
        ].into_iter().map(|rule| Regex::new(rule).expect("fixed redaction expression")).collect()
    });
    let mut out: String = text
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect();
    for rule in RULES.iter() {
        out = rule.replace_all(&out, "[redacted]").into_owned();
    }
    out
}

#[cfg(test)]
#[path = "redact_tests.rs"]
mod tests;
