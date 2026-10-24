use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;

use api::report::Draft;
use mergegate_core::report::{self, sanitize};
use serde_json::json;

use super::{failed, read, validate};
use crate::diagnostics_cmd;
use crate::output::{CliError, Outcome};

pub(super) fn run(
    title: String,
    body: PathBuf,
    logs: Option<PathBuf>,
    out: PathBuf,
) -> Result<Outcome, CliError> {
    let title = sanitize(&title);
    let summary = sanitize(&read(&body, 32 * 1024)?);
    validate(&title, &summary)?;
    let selected = logs
        .map(|p| read(&p, report::MAX_LOGS).map(|s| sanitize(&s)))
        .transpose()?;
    let snapshot = diagnostics_cmd::capture(30 * 60, false)?;
    let mut text = format!(
        "{summary}\n\n## Environment\n\n- CLI: {}\n- Platform: {}/{}\n- Hub: {}\n- Diagnostic format: 1\n",
        env!("MG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        snapshot
            .hub_version
            .as_deref()
            .unwrap_or("unavailable (local files collected)")
    );
    if let Some(logs) = selected {
        text.push_str("\n## Selected logs\n\n");
        for line in logs.lines() {
            text.push_str("    ");
            text.push_str(line);
            text.push('\n');
        }
    }
    text.push_str("\nA diagnostic ZIP was generated locally. The author can attach it manually after reviewing its contents.\n");
    validate(&title, &text)?;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&out)
        .map_err(failed)?;
    let draft = Draft {
        format_version: 1,
        id: tempfile::Builder::new()
            .prefix("report-")
            .rand_bytes(16)
            .tempfile_in(&out)
            .map_err(failed)?
            .path()
            .file_name()
            .expect("temporary file name")
            .to_string_lossy()
            .into_owned(),
        title,
    };
    write_private(
        &out.join("report.json"),
        &serde_json::to_vec_pretty(&draft).map_err(failed)?,
    )?;
    write_private(&out.join("report.md"), text.as_bytes())?;
    write_private(
        &out.join("diagnostics.json"),
        &serde_json::to_vec_pretty(&snapshot).map_err(failed)?,
    )?;
    diagnostics_cmd::zip(&out.join("diagnostics.zip"), &snapshot, Some(&text))?;
    Ok(Outcome::done(
        &json!({"id": draft.id, "repository": report::REPOSITORY, "directory": out, "draft": out.join("report.md"), "archive": out.join("diagnostics.zip"), "warnings": snapshot.warnings}),
        format!(
            "report prepared in {}\nreview report.md and diagnostics.zip, then: mg report send --from {}\nThe issue and any manually attached ZIP will be public.",
            out.display(),
            out.display()
        ),
    ))
}

pub(super) fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<(), CliError> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(failed)
}
