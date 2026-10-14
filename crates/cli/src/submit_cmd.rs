//! `mg submit`: pushes the current branch to the hub and registers it as an
//! item, or as a new revision of the branch's open item.

use api::item::{Submit, Submitted};
use api::project::Project;

use crate::client::{Client, Reply};
use crate::output::{CliError, Outcome};
use crate::project_cmd::route;
use crate::repo::{self, REMOTE};

pub fn run(title: Option<String>, wake: Option<String>) -> Result<Outcome, CliError> {
    repo::require()?;
    let client = Client::connect(&crate::home()?)?;
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
    )?;
    let request = Submit {
        project: project.name,
        branch,
        head,
        title,
        // Where `mg resume` sends a new session, and the wake command runs.
        worktree: repo::read(&["rev-parse", "--show-toplevel"])?,
        wake,
    };
    let submitted = match client.post::<Submitted>("/api/items", &request)? {
        Reply::Done(submitted) => submitted,
        Reply::Decide(decision) => return Ok(Outcome::NeedsDecision(decision)),
    };
    let text = describe(&submitted);
    Ok(Outcome::done(&submitted, text))
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

fn describe(submitted: &Submitted) -> String {
    let item = &submitted.item;
    let r = &item.revision;
    if !submitted.new_revision {
        return format!(
            "{} r{} is already at {}: {}",
            item.id,
            r.number,
            r.head.get(..12).unwrap_or(&r.head),
            item.title
        );
    }
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
    if let Some(pull) = &item.pull {
        text.push_str(&format!("\n  pull request {}", pull.url));
    }
    text
}

pub(crate) fn files(n: u32) -> String {
    match n {
        1 => "1 file".to_owned(),
        n => format!("{n} files"),
    }
}
