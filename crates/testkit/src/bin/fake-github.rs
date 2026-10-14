//! A [`FakeGithub`] as its own process, for tests and demos outside Rust: the
//! browser tests start one before their hub.
//!
//! Prints one JSON line, `{"env": {..}, "repos": "..", "web": ".."}`: the
//! variables that point a hub at it, and where to put bare clones as
//! `owner/repo.git` to serve them. Runs until stdin closes.

use std::io::{Read, Write};

use mergegate_testkit::FakeGithub;
use serde_json::{Map, Value, json};

fn main() {
    let github = FakeGithub::start();
    let env: Map<String, Value> = github
        .env()
        .into_iter()
        .map(|(key, value)| (key, Value::String(value)))
        .collect();
    let ready = json!({
        "env": env,
        "repos": github.repos(),
        "web": github.web_url(),
    });
    let mut stdout = std::io::stdout();
    writeln!(stdout, "{ready}").expect("write to stdout");
    stdout.flush().expect("flush stdout");
    // Read to EOF: the parent closing stdin, or exiting, stops the fake.
    let _ = std::io::stdin().read_to_end(&mut Vec::new());
}
