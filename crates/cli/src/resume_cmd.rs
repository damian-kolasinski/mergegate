//! `mg resume`: a prompt that brings a fresh agent session up to date on an
//! item whose last session ended (F14). It marks nothing delivered, so the
//! new session's `mg wait` still returns what the old one missed.

use api::item::ItemState;
use api::resume::{Resume, Thread};
use api::review::{Author, ReplyStatus, Verdict};
use serde::Serialize;

use crate::client::{Client, segment};
use crate::feedback_cmd::heading;
use crate::item_name;
use crate::output::{CliError, Outcome};

#[derive(Serialize)]
struct Resumed {
    #[serde(flatten)]
    resume: Resume,
    prompt: String,
}

pub fn run(item: String) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    let item = item_name::find(&client, &item)?;
    let resume: Resume = client.get(&format!("/api/items/{}/resume", segment(&item.id)))?;
    let prompt = prompt(&resume);
    let text = prompt.clone();
    Ok(Outcome::done(&Resumed { resume, prompt }, text))
}

fn prompt(resume: &Resume) -> String {
    let item = &resume.item;
    let id = &item.name;
    let mut out = format!(
        "You are picking up mergegate item {id}, \"{}\", from an agent session that ended.\n\n\
         - State: {}, at revision r{}\n\
         - Branch: {}, onto {}\n",
        item.title,
        state(item.state),
        item.revision.number,
        item.branch,
        item.target,
    );
    let closed = matches!(item.state, ItemState::Merged | ItemState::Abandoned);
    match (&resume.worktree, closed) {
        (Some(worktree), false) => out.push_str(&format!("- Worktree: {worktree} (work there)\n")),
        (Some(worktree), true) => {
            out.push_str(&format!("- Worktree: {worktree} (submitted from there)\n"));
        }
        (None, false) => {
            out.push_str("- Worktree: not recorded; use a clone with the branch checked out\n");
        }
        (None, true) => out.push_str("- Worktree: not recorded\n"),
    }
    if let (Some(wake), false) = (&resume.wake, closed) {
        out.push_str(&format!(
            "- Wake: `{wake}`, left by the session that ended. It would start that session, \
             not you: replace it with `mg submit --wake CMD` from the worktree, or remove it \
             with `mg submit --wake \"\"`.\n"
        ));
    }

    if let Some(review) = &resume.review {
        out.push_str(&format!(
            "\nThe operator's last review, on r{}: {}\n",
            review.revision,
            verdict(review.verdict)
        ));
        for line in review.summary.lines() {
            out.push_str(&format!("  {line}\n"));
        }
    }

    // A closed item's comments wait on nobody.
    if !closed && resume.open.is_empty() {
        out.push_str("\nNo comments are waiting on you.\n");
    } else if !closed {
        out.push_str("\nComments still waiting on you, oldest first:\n");
        for thread in &resume.open {
            out.push_str(&describe(thread, item.revision.number));
        }
    }

    out.push_str(&format!("\n{}", next(resume)));
    out.push_str(&format!(
        "\n\n--- How to work with mergegate ---\n\n{}",
        skill::inlined(SKILL)
    ));
    out
}

/// The skill's references a session picking up an item needs; setting up
/// and the operator's own requests are left out, as every pickup pays for
/// them.
const SKILL: &[&str] = &["references/authoring.md", "references/waiting.md"];

fn describe(thread: &Thread, newest: u32) -> String {
    let comment = &thread.comment;
    let mut out = format!("\n  {}\n", heading(comment, newest));
    for line in comment.quote.iter().flat_map(|q| q.lines()) {
        out.push_str(format!("    > {line}").trim_end());
        out.push('\n');
    }
    for line in comment.body.lines() {
        out.push_str(&format!("    {line}\n"));
    }
    for reply in &thread.replies {
        let who = match (reply.author, reply.status) {
            (Author::Operator, _) if reply.overrides => "operator (overrides you)".to_owned(),
            (Author::Operator, _) => "operator".to_owned(),
            (Author::Agent, status) => format!("you ({})", status.map_or("replied", reply_status)),
        };
        let mut lines = reply.body.lines();
        out.push_str(&format!(
            "    {who}: {}\n",
            lines.next().unwrap_or_default()
        ));
        for line in lines {
            out.push_str(&format!("      {line}\n"));
        }
    }
    out
}

fn next(resume: &Resume) -> String {
    let id = &resume.item.name;
    match resume.item.state {
        // A wait may have delivered the closing events already, so a plain
        // `mg feedback` could print nothing new. The replay repeats the
        // cleanup even when it ran, and a closed item's branch name is free
        // for new work, so the head tells the two apart.
        ItemState::Merged | ItemState::Abandoned => {
            let item = &resume.item;
            let head = &item.revision.head;
            format!(
                "The item is closed. Run `mg feedback {id} --since 0` to read how it ended. \
                 Clean up as agreed with the operator only if {} still points at r{}'s head, \
                 {}: otherwise it is done, or the name is newer work's.",
                item.branch,
                item.revision.number,
                &head[..head.len().min(12)],
            )
        }
        state => {
            let mut steps = vec![format!(
                "run `mg feedback {id}` for the events the last session didn't read"
            )];
            if !resume.open.is_empty() {
                steps.push("reply to every comment above with `mg reply`".to_owned());
            }
            if state == ItemState::ChangesRequested {
                steps.push("`mg submit` the changes from the worktree".to_owned());
            }
            steps.push(format!("wait with `mg wait {id}`"));
            let last = steps.pop().unwrap_or_default();
            format!("Next: {}, then {last}.", steps.join(", "))
        }
    }
}

fn state(state: ItemState) -> &'static str {
    match state {
        ItemState::NeedsReview => "waiting for the operator's review",
        ItemState::ChangesRequested => "changes requested",
        ItemState::Merging => "merging",
        ItemState::Merged => "merged",
        ItemState::Abandoned => "abandoned",
    }
}

fn verdict(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Comment => "commented",
        Verdict::ChangesRequested => "changes requested",
        Verdict::Approved => "approved",
    }
}

fn reply_status(status: ReplyStatus) -> &'static str {
    match status {
        ReplyStatus::Addressed => "addressed",
        ReplyStatus::Rejected => "rejected",
        ReplyStatus::Question => "question",
    }
}
