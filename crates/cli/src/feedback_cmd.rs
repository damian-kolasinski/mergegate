//! `mg feedback`: an item's new events, without waiting. `mg wait` reads and
//! prints them the same way.

use api::feedback::{Acknowledge, Acknowledged, Event, EventBody, Feedback, Source};
use api::item::ItemState;
use api::review::{Comment, Side, Verdict};

use crate::client::{Client, Reply, segment};
use crate::output::{CliError, Outcome, exit};

pub fn run(item: String, since: Option<u64>) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    let feedback = read(&client, &item, since, None)?;
    Ok(report(client, &feedback, since))
}

/// Prints and delivers what a read returned. Events exit 0 even on a closed
/// item, so the approval and the cleanup that came with a merge aren't
/// mistaken for "nothing new"; a closed item exits 3 only once nothing is
/// left to deliver.
pub(crate) fn report(client: Client, feedback: &Feedback, since: Option<u64>) -> Outcome {
    let mut text = Vec::new();
    if !feedback.events.is_empty() {
        text.push(describe(feedback));
    }
    let closed = closed(feedback.state);
    match closed {
        Some(state) => text.push(format!("{} is {state}", feedback.item)),
        None if feedback.events.is_empty() => {
            text.push(format!("no new feedback on {}", feedback.item));
        }
        None => {}
    }
    let outcome = deliver(client, feedback, since, text.join("\n\n"));
    if closed.is_some() && feedback.events.is_empty() {
        outcome.exit(exit::ITEM_CLOSED)
    } else {
        outcome
    }
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
fn deliver(client: Client, feedback: &Feedback, since: Option<u64>, text: String) -> Outcome {
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

fn describe(feedback: &Feedback) -> String {
    let mut out: Vec<String> = feedback.events.iter().map(event).collect();
    out.push(format!(
        "cursor {} (mg wait --since {} continues after it)",
        feedback.cursor, feedback.cursor
    ));
    out.join("\n\n")
}

/// One event, headed by its cursor and its type, which the skill's table of
/// events is keyed on.
pub(crate) fn event(event: &Event) -> String {
    let head = format!(
        "[{}] {} {} r{}:",
        event.cursor,
        event.body.kind(),
        event.item,
        event.revision
    );
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
            let mut out = format!("{head} the operator {verdict}");
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
            let mut out = format!("{head} {who} abandoned it");
            for line in reason.iter().flat_map(|r| r.lines()) {
                out.push_str(&format!("\n  {line}"));
            }
            out
        }
        EventBody::MergeHeld { reason } => {
            let mut out = format!("{head} the merge is held until the operator retries it");
            for line in reason.lines() {
                out.push_str(&format!("\n  {line}"));
            }
            out
        }
        EventBody::MergeCancelled {} => {
            format!("{head} the operator cancelled the held merge; it needs review again")
        }
        EventBody::MergeConflict {
            target_commit,
            paths,
            commands,
        } => {
            let short = &target_commit[..target_commit.len().min(12)];
            let mut out = format!("{head} merging conflicts with the target at {short}");
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
            let mut out = format!("{head} reviewers commented on its pull request");
            for c in comments {
                out.push_str(&comment(c, event.revision, edited.contains(&c.id)));
            }
            out
        }
        EventBody::CommentOverride { comment: c, reply } => {
            let mut out = format!(
                "{head} the operator overrides your rejection of {}; make the change",
                c.id
            );
            for line in reply.body.lines() {
                out.push_str(&format!("\n  {line}"));
            }
            out.push_str(&comment(c, event.revision, false));
            out
        }
        // A squash leaves the branch unmerged to git, so `git branch -d`
        // refuses it.
        EventBody::ItemMerged {
            commit,
            branch,
            commands,
        } => {
            let mut out = format!("{head} the operator merged it as {commit}");
            // A hub older than the cleanup commands sends none.
            if !commands.is_empty() {
                out.push_str(&format!(
                    "\n\n  Clean up {branch} as agreed with the operator; by default:"
                ));
            }
            for command in commands {
                out.push_str(&format!("\n    {command}"));
            }
            out
        }
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

#[cfg(test)]
#[path = "feedback_cmd_tests.rs"]
mod tests;
