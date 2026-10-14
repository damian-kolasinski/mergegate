//! Git over HTTP, as GitHub serves it: `git http-backend` behind Basic auth
//! with the token as the password.

use std::io::Write;
use std::process::{Command, Stdio};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use super::Shared;
use super::server::{Request, Response};

pub(super) fn handle(shared: &Shared, request: &Request) -> Response {
    let Some(repo) = repo_of(&request.path) else {
        return Response::empty(404);
    };
    let push = request.path.ends_with("/git-receive-pack")
        || request.param("service").as_deref() == Some("git-receive-pack");
    let state = shared.state();
    let Some(can_push) = shared.can_push(&state, &repo) else {
        return Response::empty(404);
    };
    if !signed_in(request, &state.token) {
        let mut response = Response::empty(401);
        response
            .headers
            .push(("WWW-Authenticate".into(), "Basic realm=\"GitHub\"".into()));
        return response;
    }
    if push && !can_push {
        return Response::empty(403);
    }
    drop(state);
    backend(shared, request)
}

/// `owner/repo` from `/owner/repo.git/...`.
fn repo_of(path: &str) -> Option<String> {
    let mut parts = path.trim_start_matches('/').splitn(3, '/');
    let owner = parts.next()?;
    let repo = parts.next()?.strip_suffix(".git")?;
    Some(format!("{owner}/{repo}"))
}

fn signed_in(request: &Request, token: &str) -> bool {
    let expected = STANDARD.encode(format!("x-access-token:{token}"));
    !token.is_empty() && request.header("authorization") == Some(&format!("Basic {expected}"))
}

fn backend(shared: &Shared, request: &Request) -> Response {
    let mut cmd = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.as_encoded_bytes().starts_with(b"GIT_") {
            cmd.env_remove(key);
        }
    }
    cmd.args(["-c", "http.receivepack=true", "http-backend"])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_PROJECT_ROOT", &shared.repos)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("PATH_INFO", &request.path)
        .env("REQUEST_METHOD", &request.method)
        .env("QUERY_STRING", &request.query)
        .env("CONTENT_LENGTH", request.body.len().to_string());
    let passed = [
        ("CONTENT_TYPE", "content-type"),
        ("HTTP_CONTENT_ENCODING", "content-encoding"),
        ("HTTP_GIT_PROTOCOL", "git-protocol"),
    ];
    for (var, header) in passed {
        if let Some(value) = request.header(header) {
            cmd.env(var, value);
        }
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("run git http-backend");
    let mut stdin = child.stdin.take().unwrap();
    let body = request.body.clone();
    // Written from another thread, since the backend may answer before it
    // has read everything.
    let writer = std::thread::spawn(move || stdin.write_all(&body));
    let output = child.wait_with_output().expect("wait for git http-backend");
    let _ = writer.join();
    cgi_response(&output.stdout)
}

/// CGI output: headers, a `Status` among them, a blank line, the body.
fn cgi_response(output: &[u8]) -> Response {
    let split = output
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| (i, i + 4))
        .or_else(|| {
            output
                .windows(2)
                .position(|w| w == b"\n\n")
                .map(|i| (i, i + 2))
        });
    let Some((end, body)) = split else {
        return Response::empty(500);
    };
    let mut response = Response {
        status: 200,
        headers: Vec::new(),
        body: output[body..].to_vec(),
    };
    for line in String::from_utf8_lossy(&output[..end]).lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("status") {
            response.status = value
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(500);
        } else {
            response.headers.push((name.into(), value.into()));
        }
    }
    response
}
