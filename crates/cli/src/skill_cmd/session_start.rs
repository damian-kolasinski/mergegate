//! `mg skill session-start`: what the session hook tells each new session,
//! and each one after a clear or a compaction. Only in a clone with a
//! `mergegate` remote: elsewhere it prints nothing, so sessions in other
//! repositories carry nothing extra. The agent gives the hook 5 seconds and
//! a failing hook must not get in a session's way, so it asks the hub once,
//! briefly, and leaves out what it can't learn.

use std::time::Duration;

use api::item::{Inbox, InboxItem, ItemState};
use serde::Serialize;
use skill::Agent;

use crate::output::Outcome;
use crate::repo::{self, REMOTE};

/// How long the hub gets to answer, well within the hook's 5 seconds.
const ASK: Duration = Duration::from_secs(2);

#[derive(Serialize)]
struct SessionStart {
    text: String,
}

pub(super) fn run(agent: Agent) -> Outcome {
    let text = context(agent).unwrap_or_default();
    Outcome::done(&SessionStart { text: text.clone() }, text)
}

fn context(agent: Agent) -> Option<String> {
    let url = repo::read(&["remote", "get-url", REMOTE]).ok()??;
    let project = project(&url)?;
    let mut out = format!(
        "This repository goes through mergegate, the operator's code review hub, as project \
         {project}. Load the mergegate skill before submitting work: `mg submit` puts it up for \
         review; never `git push` or `gh pr create`."
    );
    let branch = repo::read(&["symbolic-ref", "--quiet", "--short", "HEAD"])
        .ok()
        .flatten();
    match open_items() {
        Some(items) => {
            let here = items.iter().find(|item| {
                Some(&item.branch) == branch.as_ref()
                    && item.name.split_once('#').map(|(p, _)| p) == Some(project)
            });
            if let Some(item) = here {
                out.push_str(&format!(
                    "\n\n{name}, \"{title}\", is open on this branch, {state}. If you are \
                     picking it up, `mg resume {name}` brings you up to date.",
                    name = item.name,
                    title = item.title,
                    state = state(item.state),
                ));
            }
        }
        None => out.push_str("\n\nThe hub didn't answer just now; `mg doctor` says why."),
    }
    out.push_str("\n\n");
    out.push_str(waiting(agent));
    Some(out)
}

/// `app` of `http://127.0.0.1:7800/git/app`, the remote `mg init` adds.
fn project(url: &str) -> Option<&str> {
    match url.trim_end_matches('/').rsplit_once("/git/") {
        Some((_, project)) if !project.is_empty() && !project.contains('/') => Some(project),
        _ => None,
    }
}

/// Every open item, from the hub `mg` works through; `None` when it didn't
/// answer in time, or this `mg` can't read its answer.
fn open_items() -> Option<Vec<InboxItem>> {
    let home = crate::home().ok()?;
    let url = match home.read_connection() {
        Some(connection) => connection.url,
        None => format!("http://{}", home.read_info()?.addr),
    };
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(ASK))
        .proxy(None)
        .build()
        .into();
    let inbox: Inbox = agent
        .get(format!("{url}/api/inbox"))
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    Some([inbox.needs_you, inbox.with_agents, inbox.merging].concat())
}

fn state(state: ItemState) -> &'static str {
    match state {
        ItemState::NeedsReview => "waiting for review",
        ItemState::ChangesRequested => "with changes requested",
        ItemState::Merging => "approved and merging",
        ItemState::Merged => "merged",
        ItemState::Abandoned => "abandoned",
    }
}

/// How this agent waits for the operator's review: what `mg skill show
/// waiting` says for it, so the session needn't work out which agent it is.
fn waiting(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => {
            "After `mg submit`, wait for the review with `mg wait ITEM` run in the background \
             (the Bash tool's `run_in_background`); when it exits, read what came with \
             `mg feedback ITEM`. `mg skill show waiting` has the rest."
        }
        Agent::Codex => {
            "Never run `mg wait` here, not even in the background: Codex spends a turn on every \
             check of a running command. Submit with a wake that resumes this thread, \
             `mg submit --wake \"mg wakes codex $CODEX_THREAD_ID\"`, then end your turn. \
             `mg skill show waiting` has the rest."
        }
        Agent::OpenCode => {
            "After `mg submit`, wait for the review with `mg wait ITEM --timeout N` in a loop, \
             N just under your tool-call time limit. `mg skill show waiting` has the rest."
        }
    }
}
