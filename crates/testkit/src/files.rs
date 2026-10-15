//! Files over HTTP on localhost, as a release serves its assets: a `GET`
//! of `/{name}` answers the file `name` put in, anything else 404.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, PoisonError};

pub struct StaticFiles {
    files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    addr: String,
}

impl StaticFiles {
    pub fn start() -> StaticFiles {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a port");
        let addr = listener.local_addr().expect("bound address").to_string();
        let files = Arc::new(Mutex::new(BTreeMap::new()));
        let served = Arc::clone(&files);
        // Runs until the test process exits.
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let files = Arc::clone(&served);
                std::thread::spawn(move || {
                    let _ = handle(stream, &files);
                });
            }
        });
        StaticFiles { files, addr }
    }

    /// The URL files are served under, without a trailing slash.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn put(&self, name: &str, bytes: impl Into<Vec<u8>>) {
        self.files
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(name.into(), bytes.into());
    }
}

fn handle(stream: TcpStream, files: &Mutex<BTreeMap<String, Vec<u8>>>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    // The headers say nothing a GET of a file needs.
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim_end().is_empty() {
            break;
        }
    }
    let mut parts = line.split_whitespace();
    let found = match (parts.next(), parts.next()) {
        (Some("GET"), Some(path)) => path.strip_prefix('/').and_then(|name| {
            let files = files.lock().unwrap_or_else(PoisonError::into_inner);
            files.get(name).cloned()
        }),
        _ => None,
    };
    let (status, body) = match found {
        Some(body) => ("200 OK", body),
        None => ("404 Not Found", b"not found".to_vec()),
    };
    let mut stream = stream;
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)
}
