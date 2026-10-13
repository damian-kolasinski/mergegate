use std::io::{Read, Write};
use std::net::TcpStream;

/// A response as [`http_reply`] read it.
#[derive(Debug)]
pub struct Reply {
    pub status: u16,
    /// Names lowercased, in the order the server sent them.
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Reply {
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
    let mut stream = TcpStream::connect(addr).expect("connect to hub");
    stream.write_all(request.as_bytes()).expect("send request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    let (head, body) = response.split_once("\r\n\r\n").unwrap_or((&response, ""));
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("malformed response: {response:?}"));
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    Reply {
        status,
        headers,
        body: body.to_owned(),
    }
}
