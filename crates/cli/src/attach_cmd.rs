//! `mg attach`: adds screenshots or recordings to an item's newest revision,
//! for the operator to watch before reading the diff. `mg submit --attach`
//! does the same for the revision it submits.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use api::attachment::{Attached, Attachment};
use mergegate_core::attachment::{self, MAX_SIZE, Media, SNIFF_LEN};
use serde::Serialize;

use crate::client::{self, Client};
use crate::output::{CliError, Outcome, exit};

/// A file checked before anything is sent, so a wrong one stops a submit
/// before it pushes.
pub(crate) struct Checked {
    path: PathBuf,
    name: String,
}

#[derive(Serialize)]
struct AttachedAll<'a> {
    item: &'a str,
    attachments: &'a [Attachment],
}

pub fn run(item: String, files: Vec<PathBuf>) -> Result<Outcome, CliError> {
    let checked = check(&files)?;
    let client = Client::connect(&crate::home()?)?;
    let attachments = upload(&client, &item, None, &checked)?;
    let text = describe(&item, &attachments);
    let output = AttachedAll {
        item: &item,
        attachments: &attachments,
    };
    Ok(Outcome::done(&output, text))
}

/// Checks each file as the hub will: a file name, at most [`MAX_SIZE`], and
/// an image or video by its first bytes.
pub(crate) fn check(files: &[PathBuf]) -> Result<Vec<Checked>, CliError> {
    files.iter().map(|path| check_one(path)).collect()
}

fn check_one(path: &Path) -> Result<Checked, CliError> {
    let shown = path.display();
    let unreadable = |e: std::io::Error| {
        CliError::new(
            "attachment_unreadable",
            format!("reading {shown} failed: {e}"),
        )
    };
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| attachment::valid_name(name))
        .ok_or_else(|| {
            CliError::new(
                "invalid_name",
                format!("{shown} doesn't end in a file name"),
            )
        })?
        .to_owned();
    let mut file = File::open(path).map_err(unreadable)?;
    let size = file.metadata().map_err(unreadable)?.len();
    if size > MAX_SIZE {
        return Err(CliError::new(
            "attachment_too_large",
            format!(
                "{shown} is {}, over the {} limit",
                bytes(size),
                bytes(MAX_SIZE)
            ),
        ));
    }
    let mut head = Vec::with_capacity(SNIFF_LEN);
    file.by_ref()
        .take(SNIFF_LEN as u64)
        .read_to_end(&mut head)
        .map_err(unreadable)?;
    if Media::sniff(&head).is_none() {
        return Err(CliError::new(
            "unsupported_media",
            format!("{shown} is not a PNG, JPEG, GIF or WebP image, or an MP4 or WebM video"),
        ));
    }
    Ok(Checked {
        path: path.to_owned(),
        name,
    })
}

/// Sends the files in order, to `revision` or the newest; the rest go where
/// the first went, even if a revision lands meanwhile.
pub(crate) fn upload(
    client: &Client,
    item: &str,
    mut revision: Option<u32>,
    files: &[Checked],
) -> Result<Vec<Attachment>, CliError> {
    let path = format!("/api/items/{}/attachments", client::segment(item));
    let mut attachments = Vec::with_capacity(files.len());
    for (i, file) in files.iter().enumerate() {
        let sent = std::fs::read(&file.path)
            .map_err(|e| {
                CliError::new(
                    "attachment_unreadable",
                    format!("reading {} failed: {e}", file.path.display()),
                )
            })
            .and_then(|bytes| {
                let number = revision.map(|n| n.to_string());
                let mut query = vec![("name", file.name.as_str())];
                if let Some(number) = &number {
                    query.push(("revision", number));
                }
                client.post_bytes::<Attached>(&path, &query, bytes)
            });
        match sent {
            Ok(attached) => {
                revision = Some(attached.attachment.revision);
                attachments.push(attached.attachment);
            }
            Err(e) => return Err(unsent(e, item, &attachments, &files[i..])),
        }
    }
    Ok(attachments)
}

/// Names what went before `error`, since attaching a file again would show
/// it twice, and what's left to send: as the command that sends it when
/// sending again may work, and in the message otherwise, so a refusal such
/// as a closed item keeps the hub's hint.
fn unsent(mut error: CliError, item: &str, sent: &[Attachment], rest: &[Checked]) -> CliError {
    let paths: Vec<String> = rest.iter().map(|f| f.path.display().to_string()).collect();
    let retry = transient(&error);
    let mut notes = Vec::new();
    if !sent.is_empty() {
        let names: Vec<&str> = sent.iter().map(|a| a.name.as_str()).collect();
        notes.push(format!("{} attached before it", names.join(", ")));
    }
    // A lone file the error is about needs no naming twice.
    if !retry && (!sent.is_empty() || paths.len() > 1) {
        notes.push(format!("{} not sent", paths.join(", ")));
    }
    if !notes.is_empty() {
        error.body.message = format!("{} ({})", error.body.message, notes.join("; "));
    }
    if retry {
        error.body.hint = Some(format!("mg attach {item} {}", paths.join(" ")));
    }
    error
}

/// The upload failed on the way or in the hub's storage, not because the
/// hub refused the item or the file, nor because this mg can't read it.
fn transient(error: &CliError) -> bool {
    error.exit == exit::UNAVAILABLE
        || matches!(
            error.body.code.as_str(),
            "upload_failed" | "store_failed" | "unexpected_reply"
        )
}

pub(crate) fn describe(item: &str, attachments: &[Attachment]) -> String {
    let lines: Vec<String> = attachments
        .iter()
        .map(|a| {
            format!(
                "attached {} to {item} r{}: {}, {}",
                a.name,
                a.revision,
                a.media_type,
                bytes(a.size)
            )
        })
        .collect();
    lines.join("\n")
}

fn bytes(n: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    match n {
        n if n >= MB => format!("{:.1} MB", n as f64 / MB as f64),
        n if n >= KB => format!("{:.1} KB", n as f64 / KB as f64),
        n => format!("{n} B"),
    }
}

#[cfg(test)]
#[path = "attach_cmd_tests.rs"]
mod tests;
