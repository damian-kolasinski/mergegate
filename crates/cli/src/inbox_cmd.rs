//! `mg inbox`: open items by who has to act next.

use api::item::{Inbox, InboxItem};
use api::merge::Blocked;

use crate::client::Client;
use crate::output::{CliError, Outcome};
use crate::prioritize_cmd::name;
use crate::submit_cmd::files;

pub fn run() -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    let inbox: Inbox = client.get("/api/inbox")?;
    let text = describe(&inbox);
    Ok(Outcome::done(&inbox, text))
}

fn describe(inbox: &Inbox) -> String {
    let groups = [
        ("needs you", &inbox.needs_you),
        ("with agents", &inbox.with_agents),
        ("merging", &inbox.merging),
    ];
    let mut out = Vec::new();
    for (name, items) in groups {
        if items.is_empty() {
            continue;
        }
        out.push(format!("{name} ({})", items.len()));
        out.extend(items.iter().map(line));
    }
    if out.is_empty() {
        out.push("no open items".to_owned());
    }
    for rules in &inbox.rules {
        let changes = match rules.changes {
            1 => "1 change to its rules waits".to_owned(),
            n => format!("{n} changes to its rules wait"),
        };
        out.push(format!(
            "{}: {changes} for the operator's approval in the UI",
            rules.project
        ));
    }
    out.join("\n")
}

fn line(i: &InboxItem) -> String {
    let link = match &i.link {
        Some(link) => format!(" · {}", link.label),
        None => String::new(),
    };
    // Only when rules trusted some, so the total sets them apart.
    let files = if i.review_files == i.files {
        files(i.files)
    } else {
        format!("{} of {} to review", i.review_files, files(i.files))
    };
    let priority = match i.priority {
        Some(priority) => format!(" {}", name(priority)),
        None => String::new(),
    };
    let changed = match &i.changed_since {
        Some(since) => format!(" · {} changed since r{}", since.files, since.revision),
        None => String::new(),
    };
    let mut out = format!(
        "  {}{priority} r{}  {}  ({}{link} · {files}, +{} -{}{changed})",
        i.id, i.revision, i.title, i.branch, i.additions, i.deletions
    );
    if let Some(note) = &i.note {
        out.push_str(&format!("\n      {}", one_line(note)));
    }
    match &i.blocked {
        Some(Blocked::Held { reason }) => {
            out.push_str(&format!("\n      held: {}", one_line(reason)));
        }
        Some(Blocked::Conflict {
            target_commit,
            paths,
        }) => {
            let short = &target_commit[..target_commit.len().min(12)];
            out.push_str(&format!(
                "\n      conflicts with the target at {short}: {}",
                listed(paths)
            ));
        }
        None => {}
    }
    if !i.conflicts_with.is_empty() {
        let with = i.conflicts_with.join(", ");
        out.push_str(&format!("\n      conflicts with {with}"));
    }
    out
}

/// The first few paths, so a conflict over many keeps its item one screen
/// line; `mg feedback` lists them all.
fn listed(paths: &[String]) -> String {
    const SHOWN: usize = 3;
    let shown = paths[..paths.len().min(SHOWN)].join(", ");
    match paths.len().saturating_sub(SHOWN) {
        0 => shown,
        more => format!("{shown} and {more} more"),
    }
}

/// One line, so text written over several stays under its item.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
#[path = "inbox_cmd_tests.rs"]
mod tests;
