//! `mg events`: every item's events, hub-wide, for the operator's agent. It
//! acknowledges nothing, so the items' own agents still get theirs.

use std::io::Write;

use api::feedback::Log;

use crate::client::Client;
use crate::feedback_cmd::event;
use crate::output::{CliError, Outcome};

/// The longest one request waits; `--follow` asks again.
const REQUEST: u64 = 3600;

pub fn run(since: Option<u64>, follow: bool, json: bool) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    if !follow {
        let log = read(&client, since, 0)?;
        return Ok(Outcome::done(&log, describe(&log)));
    }
    // Without `--since` it starts from now: the first read returns at once
    // with the newest cursor.
    let mut cursor = match since {
        Some(since) => since,
        None => read(&client, None, 0)?.cursor,
    };
    let mut stdout = std::io::stdout().lock();
    loop {
        let log = read(&client, Some(cursor), REQUEST)?;
        for e in &log.events {
            let line = if json {
                serde_json::to_string(e).expect("events serialize")
            } else {
                event(e) + "\n"
            };
            // A closed pipe ends the stream: no one reads it any more, and
            // printing an outcome would fail the same way.
            if writeln!(stdout, "{line}")
                .and_then(|()| stdout.flush())
                .is_err()
            {
                std::process::exit(crate::output::exit::OK.into());
            }
        }
        cursor = log.cursor;
    }
}

fn read(client: &Client, since: Option<u64>, wait: u64) -> Result<Log, CliError> {
    let since = since.map(|cursor| cursor.to_string());
    let wait = wait.to_string();
    let mut query = vec![("wait", wait.as_str())];
    if let Some(since) = &since {
        query.push(("since", since.as_str()));
    }
    client.get_query("/api/events", &query)
}

fn describe(log: &Log) -> String {
    let mut out: Vec<String> = log.events.iter().map(event).collect();
    out.push(format!(
        "cursor {} (mg events --since {} --follow continues from here)",
        log.cursor, log.cursor
    ));
    out.join("\n\n")
}
