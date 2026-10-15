//! `mg submit`: pushes the current branch to the hub and registers it as an
//! item, or as a new revision of the branch's open item, with the
//! screenshots or recordings it shows.

use std::io::Read;
use std::path::{Path, PathBuf};

use api::attachment::Attachment;
use api::item::{Priority, Submit, Submitted};
use api::project::Project;
use clap::builder::{PossibleValuesParser, TypedValueParser};
use serde::Serialize;

use crate::attach_cmd;
use crate::client::{Client, Reply};
use crate::output::{CliError, Outcome};
use crate::project_cmd::route;
use crate::repo::{self, REMOTE};

/// [`Submitted`], and what `--attach` added.
#[derive(Serialize)]
struct Output<'a> {
    #[serde(flatten)]
    submitted: &'a Submitted,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    attachments: &'a [Attachment],
}

/// `--priority`'s values. Unlike `mg prioritize` there's no `none`: an item
/// opens without a priority anyway, and on a later submit `none` would read
/// as clearing one, which only `mg prioritize` does.
pub(crate) fn initial_priority() -> impl TypedValueParser<Value = Priority> {
    PossibleValuesParser::new(["p0", "p1", "p2", "p3"]).map(|level| match level.as_str() {
        "p0" => Priority::P0,
        "p1" => Priority::P1,
        "p2" => Priority::P2,
        _ => Priority::P3,
    })
}

pub fn run(
    title: Option<String>,
    wake: Option<String>,
    attach: Vec<PathBuf>,
    guide: Option<PathBuf>,
    priority: Option<Priority>,
    note: Option<String>,
) -> Result<Outcome, CliError> {
    repo::require()?;
    // Check and read before pushing, so a wrong file stops the submit.
    let attach = attach_cmd::check(&attach)?;
    let guide = guide.as_deref().map(read_guide).transpose()?;
    let guided = guide.is_some();
    let client = Client::connect(&crate::home()?)?;
    // Before the push, so nothing is half done; the hub refuses it too.
    if wake.is_some() && client.connected() {
        return Err(CliError::new(
            "wake_remote",
            "a wake command runs on the hub's machine, and this mg works through a hub on another machine",
        )
        .hint("submit without --wake and run: mg wait ITEM"));
    }
    let project = remote_project()?;
    let switch = "git switch -c NAME";
    // The full ref: `--short` gives `heads/NAME` when a tag shares the name.
    let branch = repo::read(&["symbolic-ref", "--quiet", "HEAD"])?
        .and_then(|head| head.strip_prefix("refs/heads/").map(str::to_owned))
        .ok_or_else(|| {
            CliError::new("detached_head", "HEAD is detached; submit from a branch").hint(switch)
        })?;
    let head = repo::read(&["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])?
        .ok_or_else(|| CliError::new("no_commits", format!("{branch} has no commits yet")))?;
    let project: Project = client.get(&route(&project)?)?;
    if branch == project.target {
        return Err(CliError::new(
            "target_branch",
            format!("{branch} is the target branch; submit work from its own branch"),
        )
        .hint(switch));
    }

    // The hub this mg talks to, even if it moved since `mg init`.
    let url = client.remote(&project.name);
    let refspec = format!("+HEAD:refs/heads/{branch}");
    repo::run(
        &format!("pushing {branch} to the hub"),
        &["push", "--quiet", &url, &refspec],
    )
    .map_err(|e| nested(&branch, e))?;
    let request = Submit {
        project: project.name,
        branch,
        head,
        title,
        // Where `mg resume` sends a new session, and the wake command runs.
        worktree: repo::read(&["rev-parse", "--show-toplevel"])?,
        wake,
        guide,
        priority,
        note,
    };
    let submitted = match client.post::<Submitted>("/api/items", &request)? {
        Reply::Done(submitted) => submitted,
        Reply::Decide(decision) => return Ok(Outcome::NeedsDecision(decision)),
    };
    let item = &submitted.item;
    let attachments = attach_cmd::upload(&client, &item.id, Some(item.revision.number), &attach)
        .map_err(|e| {
            let message = format!(
                "submitted {} r{}, but attaching failed: {}",
                item.id, item.revision.number, e.body.message
            );
            CliError {
                body: api::error::ErrorBody { message, ..e.body },
                ..e
            }
        })?;
    let mut text = describe(&submitted, guided);
    if !attachments.is_empty() {
        text.push('\n');
        text.push_str(&attach_cmd::describe(&item.id, &attachments));
    }
    let output = Output {
        submitted: &submitted,
        attachments: &attachments,
    };
    Ok(Outcome::done(&output, text))
}

/// Git can't hold two branches whose names nest (`a` and `a/b`), and the hub
/// keeps every open item's. Its refusal names the other branch, worded
/// `cannot lock ref '…': 'refs/heads/a' exists` while that branch is a loose
/// ref and `'refs/heads/a' exists; cannot create '…'` once it is packed. The
/// hub's git runs with `LC_ALL=C`, so neither is translated.
fn nested(branch: &str, error: CliError) -> CliError {
    let message = &error.body.message;
    let taken = message
        .split_once("': 'refs/heads/")
        .and_then(|(_, rest)| rest.split_once("' exists"))
        .or_else(|| {
            message
                .split_once("'refs/heads/")
                .and_then(|(_, rest)| rest.split_once("' exists; cannot create"))
        })
        .map(|(taken, _)| taken.to_owned());
    match taken {
        Some(taken) => CliError::new(
            "branch_nested",
            format!(
                "the hub has a branch {taken}, so it can't take {branch}: one branch name can't nest in another"
            ),
        )
        .hint("git branch -m NAME"),
        None => error,
    }
}

fn read_guide(path: &Path) -> Result<String, CliError> {
    let read = if path == Path::new("-") {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).map(|_| text)
    } else {
        std::fs::read_to_string(path)
    };
    read.map_err(|e| {
        CliError::new(
            "guide_unreadable",
            format!("can't read the guide {}: {e}", path.display()),
        )
    })
}

/// The project the clone's `mergegate` remote points at.
fn remote_project() -> Result<String, CliError> {
    let init = || "mg init";
    let url = repo::read(&["remote", "get-url", REMOTE])?.ok_or_else(|| {
        CliError::new("no_remote", format!("this repo has no {REMOTE} remote")).hint(init())
    })?;
    match url.trim_end_matches('/').rsplit_once("/git/") {
        Some((_, project)) if !project.is_empty() && !project.contains('/') => {
            Ok(project.to_owned())
        }
        _ => Err(CliError::new(
            "no_remote",
            format!("the {REMOTE} remote ({url}) is not a mergegate hub"),
        )
        .hint(init())),
    }
}

/// `guided` when a guide came with it, which replaces the revision's guide
/// even at the same head.
fn describe(submitted: &Submitted, guided: bool) -> String {
    let item = &submitted.item;
    let r = &item.revision;
    let mut text = if submitted.new_revision {
        let mut text = format!(
            "submitted {} r{}: {}\n  {} onto {} · {}, +{} -{}",
            item.id,
            r.number,
            item.title,
            item.branch,
            item.target,
            files(r.files),
            r.additions,
            r.deletions
        );
        if let Some(link) = &item.link {
            text.push_str(&format!("\n  {} {}", link.label, link.url));
        }
        text
    } else {
        let mut text = format!(
            "{} r{} is already at {}: {}",
            item.id,
            r.number,
            r.head.get(..12).unwrap_or(&r.head),
            item.title
        );
        if guided {
            text.push_str("\n  its guide is now the one sent");
        }
        text
    };
    for warning in &submitted.warnings {
        text.push_str(&format!("\nwarning: {warning}"));
    }
    text
}

pub(crate) fn files(n: u32) -> String {
    match n {
        1 => "1 file".to_owned(),
        n => format!("{n} files"),
    }
}
