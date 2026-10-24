//! Extract operational evidence without requiring a working database or hub.

mod archive;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use api::diagnostics::Snapshot;
use clap::Subcommand;
use serde_json::json;

use crate::client::{self, Client};
use crate::output::{CliError, Outcome};

#[derive(Subcommand)]
pub enum DiagnosticsCommand {
    /// Read recent sanitized operational records; use --json to select evidence
    Logs {
        /// Include sanitized warning/error text; inspect it before sharing
        #[arg(long)]
        details: bool,
        /// How far back to look: seconds, or 30m, 2h, 7d
        #[arg(long, default_value = "30m", value_parser = duration)]
        since: u64,
    },
    /// Save a sanitized ZIP locally; never uploads anything
    Collect {
        /// New ZIP file to create; existing files are preserved
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "30m", value_parser = duration)]
        since: u64,
    },
}

pub fn run(command: DiagnosticsCommand) -> Result<Outcome, CliError> {
    match command {
        DiagnosticsCommand::Logs { since, details } => {
            let snapshot = capture(since, details)?;
            let text = serde_json::to_string_pretty(&snapshot).map_err(failed)?;
            Ok(Outcome::done(&snapshot, text))
        }
        DiagnosticsCommand::Collect { out, since } => {
            let snapshot = capture(since, false)?;
            zip(&out, &snapshot, None)?;
            Ok(Outcome::done(
                &json!({"archive": out, "warnings": snapshot.warnings}),
                format!("diagnostics saved to {}", out.display()),
            ))
        }
    }
}

pub(super) fn capture(seconds: u64, details: bool) -> Result<Snapshot, CliError> {
    let home = crate::home()?;
    let since = now().saturating_sub(seconds);
    // Diagnostics must not invoke the normal client's automatic hub upgrade.
    let url = home
        .read_connection()
        .map(|c| c.url)
        .or_else(|| home.read_info().map(|i| format!("http://{}", i.addr)));
    let connected = url
        .ok_or_else(|| CliError::new("hub_unavailable", "the hub is not running"))
        .and_then(|url| client::reach(&url))
        .and_then(|hub| Client::to(&hub));
    let mut snapshot = match connected.and_then(|client| {
        client.time_limit(Duration::from_secs(10)).get_query(
            "/api/diagnostics",
            &[
                ("since", &since.to_string()),
                ("details", if details { "true" } else { "false" }),
            ],
        )
    }) {
        Ok(snapshot) => snapshot,
        Err(error) if home.read_connection().is_some() => return Err(error),
        Err(_) => {
            let mut snapshot = hub::diagnostics::collect_local(&home, since, details);
            snapshot
                .warnings
                .push("hub_unreachable_or_incompatible_local_files_only".into());
            snapshot
        }
    };
    sanitize_snapshot(&mut snapshot, details);
    Ok(snapshot)
}

pub(super) fn sanitize_snapshot(snapshot: &mut Snapshot, details: bool) {
    // Defense in depth for records read from disk or a connected hub.
    snapshot.hub_version = snapshot
        .hub_version
        .take()
        .map(|s| mergegate_core::report::sanitize(&s));
    for record in &mut snapshot.records {
        record.version = mergegate_core::report::sanitize(&record.version);
        record.run_id = mergegate_core::report::sanitize(&record.run_id);
        record.detail = if details {
            record
                .detail
                .take()
                .map(|s| mergegate_core::report::sanitize(&s))
        } else {
            None
        };
        record.target = mergegate_core::report::sanitize(&record.target);
        record.fields.retain(|key, _| {
            matches!(
                key.as_str(),
                "event_code"
                    | "error_code"
                    | "worker"
                    | "request_id"
                    | "method"
                    | "route"
                    | "version"
                    | "status"
                    | "elapsed_ms"
                    | "source_file"
                    | "source_line"
                    | "api_version"
            )
        });
        for value in record.fields.values_mut() {
            *value = mergegate_core::report::sanitize(value);
        }
    }
}

pub(super) fn zip(path: &Path, snapshot: &Snapshot, report: Option<&str>) -> Result<(), CliError> {
    archive::write(path, snapshot, report).map_err(failed)
}

pub(super) fn refresh_zip(path: &Path, snapshot: &Snapshot, report: &str) -> Result<(), CliError> {
    archive::replace(path, snapshot, report).map_err(failed)
}

pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub(super) fn failed(error: impl std::fmt::Display) -> CliError {
    CliError::new("diagnostics_failed", error.to_string())
}

fn duration(value: &str) -> Result<u64, String> {
    let (digits, scale) = match value.as_bytes().last() {
        Some(b's') => (&value[..value.len() - 1], 1),
        Some(b'm') => (&value[..value.len() - 1], 60),
        Some(b'h') => (&value[..value.len() - 1], 3600),
        Some(b'd') => (&value[..value.len() - 1], 86400),
        _ => (value, 1),
    };
    digits
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(scale))
        .filter(|n| *n <= 7 * 86400)
        .ok_or_else(|| "use seconds or a duration such as 30m, up to 7d".into())
}
