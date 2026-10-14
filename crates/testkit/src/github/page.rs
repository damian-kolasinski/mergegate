//! A pull request's page at its `html_url`: what a person following the
//! link from mergegate sees, so a demo or a browser test can check it.

use super::server::{Request, Response};
use super::{PullState, Shared};

pub(super) fn handle(shared: &Shared, request: &Request) -> Response {
    let segments: Vec<&str> = request.path.trim_start_matches('/').split('/').collect();
    let [owner, name, "pull", number] = segments.as_slice() else {
        return Response::empty(404);
    };
    let repo = format!("{owner}/{name}");
    let state = shared.state();
    let Some(pull) = state
        .pulls
        .iter()
        .find(|p| p.repo == repo && number.parse() == Ok(p.number))
    else {
        return Response::empty(404);
    };
    let status = match &pull.state {
        PullState::Open => "Open".to_owned(),
        PullState::Closed => "Closed".to_owned(),
        PullState::Merged { commit, .. } => format!("Merged as {}", &commit[..7]),
    };
    let html = format!(
        "<!doctype html><meta charset=utf-8><title>{title} · Pull Request #{number} · {repo}</title>\
         <body style=\"font:15px system-ui;margin:2rem\">\
         <p style=\"color:#666\">{repo} (fake GitHub)</p>\
         <h1>{title} <span style=\"color:#666\">#{number}</span></h1>\
         <p><strong>{status}</strong> · wants to merge <code>{branch}</code> into <code>{base}</code></p>",
        title = escape(&pull.title),
        branch = escape(&pull.branch),
        base = escape(&pull.base),
    );
    Response {
        status: 200,
        headers: vec![("Content-Type".into(), "text/html; charset=utf-8".into())],
        body: html.into_bytes(),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
