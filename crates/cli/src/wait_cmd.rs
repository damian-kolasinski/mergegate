//! `mg wait`: blocks until an item has events for its agent (design §6.5).

use std::time::{Duration, Instant};

use clap::ValueEnum;

use api::feedback::{Event, Feedback, Source};

use crate::client::Client;
use crate::feedback_cmd::{closed, read, report};
use crate::output::{CliError, Outcome, exit};

/// Which events end the wait.
#[derive(Clone, Copy, ValueEnum)]
pub enum Wake {
    /// Any event
    Any,
    /// Only the operator's; the wait still returns every new event
    Operator,
}

impl Wake {
    fn by(self, events: &[Event]) -> bool {
        match self {
            Self::Any => !events.is_empty(),
            Self::Operator => events.iter().any(|e| e.source == Source::Operator),
        }
    }
}

/// The longest one request waits; a longer wait asks again.
const REQUEST: u64 = 3600;

pub fn run(
    item: String,
    wake: Wake,
    timeout: u64,
    mut since: Option<u64>,
) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    let query = match wake {
        Wake::Any => "any",
        Wake::Operator => "operator",
    };
    // A timeout too long to add is as good as none.
    let deadline = (timeout > 0)
        .then(|| Instant::now().checked_add(Duration::from_secs(timeout)))
        .flatten();
    loop {
        let wait = match deadline {
            None => REQUEST,
            Some(deadline) => deadline
                .saturating_duration_since(Instant::now())
                .as_secs_f64()
                .ceil() as u64,
        };
        let feedback = read(&client, &item, since, Some((wait.min(REQUEST), query)))?;
        // Closing an item logs an event, so the agent reads who closed it
        // and why with whatever came before. The hub also answers at its
        // deadline, with events that may not be the ones this waits for.
        if closed(feedback.state).is_some() || wake.by(&feedback.events) {
            return Ok(report(client, &feedback, since, "wait"));
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            let text = format!("no feedback on {} yet; call mg wait again", feedback.item);
            return Ok(Outcome::done(&feedback, text).exit(exit::TIMEOUT));
        }
        since = next_since(since, &feedback);
    }
}

/// Where the next request reads from. A `since` past the item's newest event
/// comes back as that event's cursor, and asking from the old one again
/// would skip an event logged between the two requests under it, as cursors
/// are hub-wide. Events that didn't end the wait are asked for again, since
/// the wait returns them once it ends.
fn next_since(since: Option<u64>, feedback: &Feedback) -> Option<u64> {
    since.map(|since| {
        if feedback.events.is_empty() {
            feedback.cursor
        } else {
            since
        }
    })
}

#[cfg(test)]
#[path = "wait_cmd_tests.rs"]
mod tests;
