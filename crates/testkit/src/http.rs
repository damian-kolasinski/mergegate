use std::io::{Read, Write};
use std::net::TcpStream;

/// Sends a raw HTTP/1.1 request to `addr` and returns the status code and body.
/// Raw, so tests control every header, `Host` included.
pub fn http(addr: &str, request: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("connect to hub");
    stream.write_all(request.as_bytes()).expect("send request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    let status = response
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("malformed response: {response:?}"));
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_owned())
        .unwrap_or_default();
    (status, body)
}
