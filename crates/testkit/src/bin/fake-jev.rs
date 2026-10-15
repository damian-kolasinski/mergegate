//! A [`FakeJev`] as its own process, for the browser tests and demos.
//!
//! Takes the scores as one JSON argument, `{"scores": {path: 0-9}, "looks":
//! {path: 0-1}}`, and prints one JSON line, `{"env": {..}}`: the variables
//! that point a hub at it. Runs until stdin closes.

use std::io::{Read, Write};

use mergegate_testkit::FakeJev;
use serde_json::{Map, Value, json};

fn main() {
    let jev = FakeJev::start();
    let given: Value = std::env::args()
        .nth(1)
        .map(|arg| serde_json::from_str(&arg).expect("the argument is JSON"))
        .unwrap_or(Value::Null);
    for (path, score) in given["scores"].as_object().into_iter().flatten() {
        jev.score(path, score.as_f64().expect("a score is a number"));
    }
    for (path, noul) in given["looks"].as_object().into_iter().flatten() {
        jev.look(path, noul.as_f64().expect("a look is a number"));
    }
    let env: Map<String, Value> = jev
        .env()
        .into_iter()
        .map(|(key, value)| (key, Value::String(value)))
        .collect();
    let mut stdout = std::io::stdout();
    writeln!(stdout, "{}", json!({ "env": env })).expect("write to stdout");
    stdout.flush().expect("flush stdout");
    // Read to EOF: the parent closing stdin, or exiting, stops the fake.
    let _ = std::io::stdin().read_to_end(&mut Vec::new());
}
