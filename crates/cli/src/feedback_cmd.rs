//! `mg feedback`: an item's new events, without waiting. `mg wait` reads and
//! prints them the same way.

use api::feedback::{Acknowledge, Acknowledged, Event, EventBody, Feedback, Source};
use api::item::ItemState;
use api::review::{Comment, Side, Verdict};

use crate::client::{Client, Reply, segment};
use crate::output::{CliError, Outcome};

pub fn run(item: String, since: Option<u64>) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    let feedback = read(&client, &item, since, None)?;
    let text = if feedback.events.is_empty() {
        format!("no new feedback on {}", feedback.item)
    } else {
        describe(&feedback)
    };
    Ok(deliver(client, &feedback, since, text))
}

/// The events after `since` (or after the delivered cursor). With `wait`,
/// the hub holds the request for up to that many seconds until one arrives.
pub(crate) fn read(
    client: &Client,
    item: &str,
    since: Option<u64>,
    wait: Option<(u64, &str)>,
) -> Result<Feedback, CliError> {
    let since = since.map(|cursor| cursor.to_string());
    let mut query = Vec::new();
    if let Some(since) = &since {
        query.push(("since", since.as_str()));
    }
    let seconds;
    if let Some((wait, wake)) = wait {
        seconds = wait.to_string();
        query.push(("wait", seconds.as_str()));
        query.push(("for", wake));
    }
    client.get_query(&format!("/api/items/{}/feedback", segment(item)), &query)
}

/// Prints `feedback`, then acknowledges its events unless they are a replay.
pub(crate) fn deliver(
    client: Client,
    feedback: &Feedback,
    since: Option<u64>,
    text: String,
) -> Outcome {
    let outcome = Outcome::done(feedback, text);
    // A replay may start past the delivered cursor, and acknowledging it
    // would skip the events before `since` that were never printed.
    if feedback.events.is_empty() || since.is_some() {
        return outcome;
    }
    let path = format!("/api/items/{}/feedback/ack", segment(&feedback.item));
    let ack = Acknowledge {
        cursor: feedback.cursor,
    };
    // A failed acknowledgement only means the events come again.
    outcome.then(move || {
        let _: Result<Reply<Acknowledged>, _> = client.post(&path, &ack);
    })
}

pub(crate) fn closed(state: ItemState) -> Option<&'static str> {
    match state {
        ItemState::Merged => Some("merged"),
        ItemState::Abandoned => Some("abandoned"),
        _ => None,
    }
}

pub(crate) fn describe(feedback: &Feedback) -> String {
    let mut out: Vec<String> = feedback.events.iter().map(event).collect();
    out.push(format!(
        "cursor {} (mg wait --since {} replays from here)",
        feedback.cursor, feedback.cursor
    ));
    out.join("\n\n")
}

pub(crate) fn event(event: &Event) -> String {
    match &event.body {
        EventBody::ReviewSubmitted {
            verdict,
            summary,
            comments,
            replies,
        } => {
            let verdict = match verdict {
                Verdict::Comment => "commented",
                Verdict::ChangesRequested => "requested changes",
                Verdict::Approved => "approved",
            };
            let mut out = format!(
                "[{}] {} r{}: the operator {verdict}",
                event.cursor, event.item, event.revision
            );
            for line in summary.lines() {
                out.push_str(&format!("\n  {line}"));
            }
            for c in comments {
                out.push_str(&comment(c, event.revision, false));
            }
            for r in replies {
                out.push_str(&format!("\n\n  reply to {}", r.comment));
                for line in r.body.lines() {
                    out.push_str(&format!("\n    {line}"));
                }
            }
            out
        }
        EventBody::ItemAbandoned { reason } => {
            let who = match event.source {
                Source::Operator => "the operator",
                Source::Agent => "its agent",
                Source::External => "GitHub",
            };
            let mut out = format!(
                "[{}] {} r{}: {who} abandoned it",
                event.cursor, event.item, event.revision
            );
            for line in reason.iter().flat_map(|r| r.lines()) {
                out.push_str(&format!("\n  {line}"));
            }
            out
        }
        EventBody::MergeHeld { reason } => {
            let mut out = format!(
                "[{}] {} r{}: the merge is held until the operator retries it",
                event.cursor, event.item, event.revision
            );
            for line in reason.lines() {
                out.push_str(&format!("\n  {line}"));
            }
            out
        }
        EventBody::MergeCancelled {} => format!(
            "[{}] {} r{}: the operator cancelled the held merge; it needs review again",
            event.cursor, event.item, event.revision
        ),
        EventBody::MergeConflict {
            target_commit,
            paths,
            commands,
        } => {
            let short = &target_commit[..target_commit.len().min(12)];
            let mut out = format!(
                "[{}] {} r{}: merging conflicts with the target at {short}",
                event.cursor, event.item, event.revision
            );
            for path in paths {
                out.push_str(&format!("\n  {path}"));
            }
            out.push_str("\n\n  Rebase and submit again:");
            for command in commands {
                out.push_str(&format!("\n    {command}"));
            }
            out
        }
        EventBody::ExternalComments { comments, edited } => {
            let mut out = format!(
                "[{}] {} r{}: reviewers commented on its pull request",
                event.cursor, event.item, event.revision
            );
            for c in comments {
                out.push_str(&comment(c, event.revision, edited.contains(&c.id)));
            }
            out
        }
        EventBody::CommentOverride { comment: c, reply } => {
            let mut out = format!(
                "[{}] {} r{}: the operator overrides your rejection of {}; make the change",
                event.cursor, event.item, event.revision, c.id
            );
            for line in reply.body.lines() {
                out.push_str(&format!("\n  {line}"));
            }
            out.push_str(&comment(c, event.revision, false));
            out
        }
        // A squash leaves the branch unmerged to git, so `git branch -d`
        // refuses it.
        EventBody::ItemMerged { commit, branch } => format!(
            "[{}] {} r{}: the operator merged it as {commit}\n\n  \
             Clean up {branch} as agreed with the operator; by default:\n    \
             git switch --detach\n    git branch -D {branch}",
            event.cursor, event.item, event.revision
        ),
    }
}

/// A comment under an event: where it is, who left it if not the operator,
/// the lines it quotes and its body.
fn comment(c: &Comment, revision: u32, edited: bool) -> String {
    let mut out = format!("\n\n  {}", heading(c, revision));
    if edited {
        out.push_str(" (edited)");
    }
    for line in c.quote.iter().flat_map(|q| q.lines()) {
        out.push_str(format!("\n    > {line}").trim_end());
    }
    for line in c.body.lines() {
        out.push_str(&format!("\n    {line}"));
    }
    out
}

/// `c_7 path:line`, and who left a comment from GitHub.
pub(crate) fn heading(c: &Comment, revision: u32) -> String {
    let mut out = format!("{} {}", c.id, place(c, revision));
    if let Some(external) = &c.external {
        let bot = if external.bot { " (bot)" } else { "" };
        out.push_str(&format!(", from {}{bot} on GitHub", external.author));
    }
    out
}

/// `path:line`, or `path:start-line`, naming the old side and a revision
/// other than the event's. A comment from GitHub on lines no revision's diff
/// shows names only its file, and one on no file the pull request.
fn place(comment: &Comment, revision: u32) -> String {
    let mut out = match (
        &comment.path,
        comment.side,
        comment.start_line,
        comment.line,
    ) {
        (Some(path), Some(side), Some(start_line), Some(line)) => {
            let mut out = format!("{path}:");
            if start_line != line {
                out.push_str(&format!("{start_line}-"));
            }
            out.push_str(&line.to_string());
            if side == Side::Old {
                out.push_str(" (old)");
            }
            out
        }
        _ => match comment.external.as_ref().and_then(|e| e.path.as_deref()) {
            Some(path) => format!("{path} (on lines the diff doesn't show)"),
            None => "the pull request".to_owned(),
        },
    };
    if comment.revision != revision {
        out.push_str(&format!(" @r{}", comment.revision));
    }
    out
}
