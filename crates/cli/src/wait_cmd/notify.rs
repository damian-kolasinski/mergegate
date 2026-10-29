//! A wait in Claude Code says that events came and leaves them undelivered.
//! Claude Code can lose a background command's output, as when a session is
//! reattached; a wait that printed and acknowledged events there loses them
//! with it. `mg feedback`, which the agent runs in the foreground, prints and
//! delivers them instead, and is also what the agent runs after a wait that
//! ended without output, so both paths read the same events.

use serde::Serialize;

use api::feedback::Feedback;
use api::item::ItemState;

use crate::output::Outcome;

/// What a wait that leaves its events undelivered prints.
#[derive(Debug, Serialize)]
struct Arrived<'a> {
    item: &'a str,
    name: &'a str,
    state: ItemState,
    /// The undelivered events, oldest first.
    pending: Vec<Pending>,
    /// The command that prints and delivers them.
    read: String,
}

#[derive(Debug, Serialize)]
struct Pending {
    cursor: u64,
    #[serde(rename = "type")]
    kind: &'static str,
}

/// `feedback`'s events as news, with the `mg feedback` that reads them after
/// `since`, as the wait did.
pub(crate) fn arrived(feedback: &Feedback, since: Option<u64>) -> Outcome {
    let mut read = format!("mg feedback {}", feedback.name);
    if let Some(since) = since {
        read.push_str(&format!(" --since {since}"));
    }
    let arrived = Arrived {
        item: &feedback.item,
        name: &feedback.name,
        state: feedback.state,
        pending: feedback
            .events
            .iter()
            .map(|e| Pending {
                cursor: e.cursor,
                kind: e.body.kind(),
            })
            .collect(),
        read,
    };
    Outcome::done(&arrived, text(&arrived))
}

fn text(arrived: &Arrived) -> String {
    let events: Vec<String> = arrived
        .pending
        .iter()
        .map(|p| format!("[{}] {}", p.cursor, p.kind))
        .collect();
    format!(
        "{} has new events: {}. Nothing is marked delivered; read and act on them with: {}",
        arrived.name,
        events.join(", "),
        arrived.read
    )
}

#[cfg(test)]
#[path = "notify_tests.rs"]
mod tests;
