use std::collections::BTreeMap;

use api::diagnostics::Field as Value;
use tracing::field::{Field, Visit};

#[derive(Default)]
pub(super) struct Fields(pub BTreeMap<String, Value>, pub Option<String>);

struct Limited(String);

impl std::fmt::Write for Limited {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let mut end = text.len().min(4096_usize.saturating_sub(self.0.len()));
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        self.0.push_str(&text[..end]);
        if end < text.len() {
            Err(std::fmt::Error)
        } else {
            Ok(())
        }
    }
}

impl Fields {
    fn detail(&mut self, args: std::fmt::Arguments<'_>) {
        let mut limited = Limited(String::new());
        let _ = std::fmt::write(&mut limited, args);
        let mut text = mergegate_core::report::sanitize(&limited.0);
        let mut end = text.len().min(1024);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        self.1 = Some(text);
    }

    fn put(&mut self, field: &Field, value: &str) {
        let safe = match field.name() {
            "method" => matches!(
                value,
                "GET"
                    | "POST"
                    | "PUT"
                    | "PATCH"
                    | "DELETE"
                    | "HEAD"
                    | "OPTIONS"
                    | "CONNECT"
                    | "TRACE"
            ),
            "route" => {
                value.len() <= 160
                    && value
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"/_-{}.*".contains(&c))
            }
            "event_code" | "error_code" | "worker" | "request_id" | "version" | "source_file" => {
                value.len() <= 128
                    && value
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"._-+".contains(&c))
            }
            _ => false,
        };
        if safe {
            self.0
                .insert(field.name().into(), Value::Text(value.into()));
        }
    }

    fn put_number(&mut self, field: &Field, value: u64) {
        if matches!(
            field.name(),
            "status" | "elapsed_ms" | "source_line" | "api_fingerprint" | "count"
        ) {
            self.0.insert(field.name().into(), Value::Number(value));
        }
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.detail(format_args!("{value}"));
        } else {
            self.put(field, value);
        }
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.put_number(field, value);
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        if let Ok(value) = u64::try_from(value) {
            self.put_number(field, value);
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.detail(format_args!("{value:?}"));
        }
    }
}

#[cfg(test)]
#[path = "fields_tests.rs"]
mod tests;
