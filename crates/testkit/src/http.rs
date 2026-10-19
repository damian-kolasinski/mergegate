use std::io::{Read, Write};
use std::net::TcpStream;

/// A response as [`http_reply`] read it, or [`http_bytes`] with its body's
/// bytes.
#[derive(Debug)]
pub struct Reply<B = String> {
    pub status: u16,
    /// Names lowercased, in the order the server sent them.
    pub headers: Vec<(String, String)>,
    pub body: B,
}

impl<B> Reply<B> {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Sends a raw HTTP/1.1 request to `addr` and returns the status code and body.
/// Raw, so tests control every header, `Host` included.
pub fn http(addr: &str, request: &str) -> (u16, String) {
    let reply = http_reply(addr, request);
    (reply.status, reply.body)
}

/// [`http`], keeping the response headers.
pub fn http_reply(addr: &str, request: &str) -> Reply {
    let reply = http_bytes(addr, request);
    Reply {
        status: reply.status,
        headers: reply.headers,
        body: String::from_utf8(reply.body).expect("a text body"),
    }
}

/// [`http_reply`] for a body that may not be text, such as a zip's.
pub fn http_bytes(addr: &str, request: &str) -> Reply<Vec<u8>> {
    let mut stream = TcpStream::connect(addr).expect("connect to hub");
    stream.write_all(request.as_bytes()).expect("send request");
    let mut response = Vec::new();
    stream.read_to_end(&mut response).expect("read response");
    let split = response.windows(4).position(|w| w == b"\r\n\r\n");
    let (head, body) = match split {
        Some(at) => (&response[..at], &response[at + 4..]),
        None => (&response[..], &[][..]),
    };
    let head = String::from_utf8_lossy(head);
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("malformed response: {head:?}"));
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    Reply {
        status,
        headers,
        body: body.to_vec(),
    }
}
