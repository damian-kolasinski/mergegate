use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use api::report::{Draft, Receipt};
use backend_github::GithubClient;
use mergegate_core::report::{self, IssueReporter, Report, ReportError, sanitize};
use serde_json::json;

use super::{failed, read, validate};
use crate::output::{CliError, Outcome};

pub(super) fn run(from: PathBuf, retry: bool) -> Result<Outcome, CliError> {
    let draft: Draft =
        serde_json::from_str(&read(&from.join("report.json"), 4096)?).map_err(failed)?;
    if draft.format_version != 1
        || draft.id.is_empty()
        || draft.id.len() > 64
        || !draft
            .id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(CliError::new(
            "invalid_report",
            "unsupported report format or ID",
        ));
    }
    let lock = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(from.join("send.lock"))
        .map_err(failed)?;
    lock.try_lock()
        .map_err(|_| CliError::new("report_busy", "another process is sending this report"))?;
    let receipt_path = from.join("issue.json");
    if receipt_path.exists() {
        let receipt = serde_json::from_str(&read(&receipt_path, 4096)?).map_err(failed)?;
        return Ok(outcome(&from, &receipt));
    }
    let title = sanitize(&draft.title);
    let body = sanitize(&read(&from.join("report.md"), report::MAX_BODY)?);
    validate(&title, &body)?;
    let pending = from.join("send.pending");
    let recovering = pending.exists();
    if recovering && !retry {
        return Err(CliError::new("report_uncertain", "an earlier send may have created the issue")
            .hint("check the mergegate issue tracker, then use --retry to reconcile by report ID before sending again"));
    }
    let mut snapshot: api::diagnostics::Snapshot =
        serde_json::from_str(&read(&from.join("diagnostics.json"), 8 * 1024 * 1024)?)
            .map_err(failed)?;
    crate::diagnostics_cmd::sanitize_snapshot(&mut snapshot, false);
    crate::diagnostics_cmd::refresh_zip(&from.join("diagnostics.zip"), &snapshot, &body)?;
    let reporter = GithubClient::from_env();
    let send_error = |error: ReportError| {
        CliError::new("report_send_failed", sanitize(&error.to_string())).hint(format!(
            "draft and ZIP remain in {}; check GitHub before retrying with --retry",
            from.display()
        ))
    };
    let issue = match reporter.find(&draft.id).map_err(send_error)? {
        Some(issue) => issue,
        None if recovering => {
            // GitHub's issue list can lag immediately after a successful
            // create. Recovery must never turn an absent result into a POST.
            return Err(CliError::new("report_uncertain", "GitHub has not listed this report yet; no new issue was created")
                .hint("wait and retry with --retry; if GitHub definitively rejected the original report, prepare a new draft after checking the tracker"));
        }
        None => {
            super::prepare::write_private(&pending, b"The outcome of a send may be unknown.\n")?;
            match reporter.create(&Report {
                id: &draft.id,
                title: &title,
                body: &body,
            }) {
                Ok(issue) => issue,
                Err(error) => {
                    if matches!(error, ReportError::NotSent(_)) {
                        let _ = fs::remove_file(&pending);
                    }
                    return Err(send_error(error));
                }
            }
        }
    };
    let receipt = Receipt {
        number: issue.number,
        url: issue.url,
    };
    let mut temporary = tempfile::NamedTempFile::new_in(&from).map_err(failed)?;
    temporary
        .write_all(&serde_json::to_vec_pretty(&receipt).map_err(failed)?)
        .map_err(failed)?;
    temporary.persist(&receipt_path).map_err(|e| {
        failed(e).hint(format!(
            "issue created at {}; do not send again without checking it",
            receipt.url
        ))
    })?;
    let _ = fs::remove_file(pending);
    Ok(outcome(&from, &receipt))
}

fn outcome(from: &std::path::Path, receipt: &Receipt) -> Outcome {
    let archive = from.join("diagnostics.zip");
    Outcome::done(
        &json!({"issue": receipt, "archive": archive, "repository": report::REPOSITORY}),
        format!(
            "{}\nattach {} manually after reviewing its contents",
            receipt.url,
            archive.display()
        ),
    )
}
