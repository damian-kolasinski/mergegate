//! Jev on localhost: `POST /v1/systemone`, answering each file's questions
//! with the score and look the test gave its path.
//!
//! Point a hub at it with [`FakeJev::env`]. A file's path is read from the
//! file map the judge puts in its state, as in `3. src/login.rs (modified`.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{Map, Value, json};

/// The key the fake accepts.
pub const JEV_KEY: &str = "ts_fake_key";

pub struct FakeJev {
    shared: Arc<Shared>,
    addr: String,
}

struct Shared {
    state: Mutex<State>,
    stopping: AtomicBool,
}

#[derive(Default)]
struct State {
    /// By path: 0 to 9, as Jev scores; 4 for a path with none.
    scores: BTreeMap<String, f64>,
    /// By path: 0 to 1, whether a trusted file needs a look; 0 for none.
    looks: BTreeMap<String, f64>,
    /// Requests still to answer busy.
    busy: u32,
    /// Each request's body, in order.
    requests: Vec<Value>,
}

impl FakeJev {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake Jev");
        let addr = listener.local_addr().unwrap().to_string();
        let shared = Arc::new(Shared {
            state: Mutex::new(State::default()),
            stopping: AtomicBool::new(false),
        });
        let serving = shared.clone();
        std::thread::spawn(move || serve(listener, &serving));
        Self { shared, addr }
    }

    /// The variables that point a hub at this fake, with its key set.
    pub fn env(&self) -> Vec<(String, String)> {
        vec![
            ("MG_JEV_URL".into(), format!("http://{}", self.addr)),
            ("MG_JEV_BACKOFF_MS".into(), "1".into()),
            ("TYPESAFE_API_KEY".into(), JEV_KEY.into()),
        ]
    }

    pub fn score(&self, path: &str, score: f64) -> &Self {
        self.state().scores.insert(path.into(), score);
        self
    }

    pub fn look(&self, path: &str, noul: f64) -> &Self {
        self.state().looks.insert(path.into(), noul);
        self
    }

    /// Answers the next `requests` with 529, as Jev does when overloaded.
    pub fn busy(&self, requests: u32) {
        self.state().busy = requests;
    }

    /// Every request's body so far.
    pub fn requests(&self) -> Vec<Value> {
        self.state().requests.clone()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.shared.state.lock().unwrap()
    }
}

impl Drop for FakeJev {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        // Wakes the accept loop so it sees it should stop.
        let _ = TcpStream::connect(&self.addr);
    }
}

fn serve(listener: TcpListener, shared: &Arc<Shared>) {
    for stream in listener.incoming() {
        if shared.stopping.load(Ordering::SeqCst) {
            return;
        }
        let Ok(stream) = stream else { continue };
        let shared = shared.clone();
        std::thread::spawn(move || {
            let _ = handle(stream, &shared);
        });
    }
}

fn handle(mut stream: TcpStream, shared: &Shared) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut length = 0;
    let mut key = String::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header)?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            match name.to_ascii_lowercase().as_str() {
                "content-length" => length = value.trim().parse().unwrap_or(0),
                "authorization" => key = value.trim().trim_start_matches("Bearer ").into(),
                _ => {}
            }
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let (status, reply) = if !line.starts_with("POST /v1/systemone ") {
        (404, json!({ "error": { "message": "not found" } }))
    } else if key != JEV_KEY {
        (401, json!({ "error": { "message": "invalid API key" } }))
    } else {
        answer(shared, serde_json::from_slice(&body).unwrap_or(Value::Null))
    };
    let body = reply.to_string();
    write!(
        stream,
        "HTTP/1.1 {status} Fake\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    stream.flush()
}

fn answer(shared: &Shared, request: Value) -> (u16, Value) {
    let mut state = shared.state.lock().unwrap();
    state.requests.push(request.clone());
    if state.busy > 0 {
        state.busy -= 1;
        return (529, json!({ "error": { "message": "overloaded" } }));
    }
    let paths = file_map(request["state"].as_str().unwrap_or_default());
    let mut answers = Map::new();
    for key in request["questions"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, _)| k)
    {
        let Some((n, kind)) = key.strip_prefix('f').and_then(|k| k.split_once('_')) else {
            continue;
        };
        let path = paths.get(n).cloned().unwrap_or_default();
        let answer = match kind {
            "importance" => json!({
                "type": "score",
                "score": state.scores.get(&path).copied().unwrap_or(4.0),
            }),
            _ => json!({
                "type": "noul",
                "noul": state.looks.get(&path).copied().unwrap_or(0.0),
            }),
        };
        answers.insert(key.clone(), answer);
    }
    (200, json!({ "answers": answers }))
}

/// File numbers to paths.
fn file_map(state: &str) -> BTreeMap<String, String> {
    state
        .lines()
        .filter_map(|line| {
            let (n, rest) = line.split_once(". ")?;
            if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let (path, _) = rest.rsplit_once(" (")?;
            Some((n.to_owned(), path.to_owned()))
        })
        .collect()
}
