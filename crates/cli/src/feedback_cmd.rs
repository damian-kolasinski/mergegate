//! `mg feedback`: an item's new events for its agent.

use api::feedback::{Acknowledge, Acknowledged, Event, EventBody, Feedback};
use api::review::Verdict;

use crate::client::{Client, Reply, segment};
use crate::output::{CliError, Outcome};

pub fn run(item: String, since: Option<u64>) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    let feedback = read(&client, &item, since)?;
    let text = if feedback.events.is_empty() {
        format!("no new feedback on {}", feedback.item)
    } else {
        describe(&feedback)
    };
    Ok(deliver(client, &feedback, text))
}

/// The events after `since` (or after the delivered cursor).
fn read(client: &Client, item: &str, since: Option<u64>) -> Result<Feedback, CliError> {
    let since = since.map(|cursor| cursor.to_string());
    let mut query = Vec::new();
    if let Some(since) = &since {
        query.push(("since", since.as_str()));
    }
    client.get_query(&format!("/api/items/{}/feedback", segment(item)), &query)
}

/// Prints `feedback`, then acknowledges its events.
fn deliver(client: Client, feedback: &Feedback, text: String) -> Outcome {
    let outcome = Outcome::done(feedback, text);
    if feedback.events.is_empty() {
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

fn describe(feedback: &Feedback) -> String {
    let mut out: Vec<String> = feedback.events.iter().map(event).collect();
    out.push(format!(
        "cursor {} (mg feedback --since {} replays from here)",
        feedback.cursor, feedback.cursor
    ));
    out.join("\n\n")
}

fn event(event: &Event) -> String {
    match &event.body {
        EventBody::ReviewSubmitted { verdict, summary } => {
            let verdict = match verdict {
                Verdict::Comment => "commented",
                Verdict::ChangesRequested => "requested changes",
            };
            let mut out = format!(
                "[{}] {} r{}: the operator {verdict}",
                event.cursor, event.item, event.revision
            );
            for line in summary.lines() {
                out.push_str(&format!("\n  {line}"));
            }
            out
        }
    }
}
