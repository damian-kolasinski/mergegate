//! Prepare evidence locally, then explicitly publish a product issue.

mod prepare;
mod send;

use std::io::Read;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use mergegate_core::report;

use crate::output::{CliError, Outcome};

#[derive(Subcommand)]
pub enum ReportCommand {
    /// Prepare a sanitized issue draft and diagnostics.zip; nothing is uploaded
    Prepare {
        #[arg(long)]
        title: String,
        /// UTF-8 summary: problem, expected/actual behavior and reproduction steps
        #[arg(long, value_name = "FILE")]
        body: PathBuf,
        /// Optional UTF-8 log excerpts selected for this issue (up to 12 KiB)
        #[arg(long, value_name = "FILE")]
        logs: Option<PathBuf>,
        /// New private directory for report.json, report.md and diagnostics.zip
        #[arg(long)]
        out: PathBuf,
    },
    /// Publish the reviewed draft to damian-kolasinski/mergegate using your gh login
    Send {
        /// Directory produced by prepare; inspect report.md and the ZIP first
        #[arg(long)]
        from: PathBuf,
        /// Reconcile and retry an interrupted/failed send after checking GitHub
        #[arg(long)]
        retry: bool,
    },
}

pub fn run(command: ReportCommand) -> Result<Outcome, CliError> {
    match command {
        ReportCommand::Prepare {
            title,
            body,
            logs,
            out,
        } => prepare::run(title, body, logs, out),
        ReportCommand::Send { from, retry } => send::run(from, retry),
    }
}

fn read(path: &Path, limit: usize) -> Result<String, CliError> {
    let mut value = String::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(limit as u64 + 1).read_to_string(&mut value))
        .map_err(failed)?;
    if value.len() > limit {
        return Err(CliError::new(
            "report_too_large",
            format!("{} exceeds {limit} bytes", path.display()),
        ));
    }
    Ok(value)
}

fn validate(title: &str, body: &str) -> Result<(), CliError> {
    if title.trim().is_empty()
        || title.len() > report::MAX_TITLE
        || title.contains(['\n', '\r'])
        || body.trim().is_empty()
        || body.len() > report::MAX_BODY
    {
        return Err(CliError::new(
            "invalid_report",
            "a report needs a single-line title up to 200 bytes and a nonempty body up to 48 KiB",
        ));
    }
    Ok(())
}

fn failed(error: impl std::fmt::Display) -> CliError {
    CliError::new("report_failed", error.to_string())
}
