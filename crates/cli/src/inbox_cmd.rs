//! `mg inbox`: open items by who has to act next.

use api::item::{Inbox, InboxItem};

use crate::client::Client;
use crate::output::{CliError, Outcome};
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
        return "no open items".to_owned();
    }
    out.join("\n")
}

fn line(i: &InboxItem) -> String {
    let link = match &i.link {
        Some(link) => format!(" · {}", link.label),
        None => String::new(),
    };
    // Only when trusted files leave lines out, which the total then shows.
    let review = i.review_additions + i.review_deletions;
    let review = if review == i.additions + i.deletions {
        String::new()
    } else {
        let noun = if review == 1 { "line" } else { "lines" };
        format!("{review} {noun} to review, ")
    };
    format!(
        "  {} r{}  {}  ({}{link} · {}, {review}+{} -{})",
        i.id,
        i.revision,
        i.title,
        i.branch,
        files(i.files),
        i.additions,
        i.deletions
    )
}
