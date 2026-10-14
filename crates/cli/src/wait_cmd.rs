//! `mg wait`: blocks until an item has events for its agent (design §6.5).

use std::time::{Duration, Instant};

use clap::ValueEnum;

use api::feedback::{Event, Source};

use crate::client::Client;
use crate::feedback_cmd::{closed, deliver, describe, read};
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
    since: Option<u64>,
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
        // The hub also answers at its deadline, with events that may not be
        // the ones this waits for.
        if wake.by(&feedback.events) {
            let text = describe(&feedback);
            return Ok(deliver(client, &feedback, since, text));
        }
        if let Some(state) = closed(feedback.state) {
            let text = format!("{} is {state}", feedback.item);
            return Ok(Outcome::done(&feedback, text).exit(exit::ITEM_CLOSED));
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            let text = format!("no feedback on {} yet; call mg wait again", feedback.item);
            return Ok(Outcome::done(&feedback, text).exit(exit::TIMEOUT));
        }
    }
}
