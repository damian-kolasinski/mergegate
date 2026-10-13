//! The UI's live updates (F6): `GET /api/events/ui` tells open pages when the
//! inbox changes, and stopping the hub ends the stream.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use testkit::{Env, TestRepo};

fn env() -> Env {
    Env::new(env!("CARGO_BIN_EXE_mg"))
}

/// Opens the stream and reads past the response head. The hub subscribes
/// before it answers, so every event after this returns is seen.
fn open(env: &Env) -> BufReader<TcpStream> {
    let addr = env.hub_addr();
    let mut stream = TcpStream::connect(&addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(
        stream,
        "GET /api/events/ui HTTP/1.1\r\nHost: {addr}\r\nAccept: text/event-stream\r\n\r\n"
    )
    .unwrap();
    let mut reader = BufReader::new(stream);
    let mut head = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        head.push(line.trim_end().to_ascii_lowercase());
    }
    assert!(head[0].starts_with("http/1.1 200"), "{head:?}");
    assert!(head.contains(&"content-type: text/event-stream".to_owned()));
    reader
}

/// The next event's data. Chunk-size lines and keep-alive comments around it
/// are skipped.
fn next_data(reader: &mut BufReader<TcpStream>) -> String {
    loop {
        let mut line = String::new();
        assert_ne!(reader.read_line(&mut line).unwrap(), 0, "stream ended");
        if let Some(data) = line.strip_prefix("data: ") {
            return data.trim_end().to_owned();
        }
    }
}

#[test]
fn f6_a_submit_tells_open_pages_the_inbox_changed() {
    let env = env();
    env.start_hub();
    let repo = TestRepo::new("app");
    env.mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    env.mg_at(repo.root(), &["--json", "init"]).assert_code(0);
    let mut first = open(&env);
    let mut second = open(&env);

    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Add the parser", &[("src/parser.rs", "fn parse() {}\n")]);
    env.mg_at(repo.root(), &["--json", "submit"]).assert_code(0);

    assert_eq!(next_data(&mut first), r#"{"type":"inbox.changed"}"#);
    assert_eq!(next_data(&mut second), r#"{"type":"inbox.changed"}"#);
}

#[test]
fn stopping_the_hub_ends_open_streams() {
    let env = env();
    env.start_hub();
    let mut stream = open(&env);

    env.mg(&["--json", "hub", "stop"]).assert_code(0);

    // Read to the end: a stream the hub kept open would hit the timeout.
    let mut rest = Vec::new();
    stream.read_to_end(&mut rest).unwrap();
}
