//! Reactions on pull requests' descriptions, as review bots leave them.

use serde_json::{Value, json};

use super::State;
use super::comments::{secs, stamp};
use super::server::{Request, Response};

/// One account's reaction on pull request `pull` of `repo`; `content` as
/// GitHub spells it, as in `+1` or `eyes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Reaction {
    pub repo: String,
    pub pull: u64,
    pub login: String,
    pub content: String,
}

/// Every account a User, as GitHub types Codex's connector on reactions.
pub(super) fn list(state: &State, repo: &str, pull: &str) -> Response {
    let pull: u64 = pull.parse().unwrap_or(0);
    let reactions: Vec<Value> = state
        .reactions
        .iter()
        .enumerate()
        .filter(|(_, (r, _))| r.repo == repo && r.pull == pull)
        .map(|(id, (r, at))| {
            json!({
                "id": id + 1,
                "user": { "login": r.login, "type": "User" },
                "content": r.content,
                "created_at": stamp(*at),
            })
        })
        .collect();
    Response::json(200, &Value::Array(reactions))
}

/// Adds the reaction, or with `on` false takes it back; GitHub keeps one of
/// each content per account.
pub(super) fn set(state: &mut State, reaction: Reaction, on: bool) {
    state.reactions.retain(|(r, _)| *r != reaction);
    if on {
        state.reactions.push((reaction, secs()));
    }
}

/// `POST /_fake/reactions` with `{repo, pull, login, content}` adds one,
/// `DELETE` takes it back.
pub(super) fn control(state: &mut State, request: &Request) -> Response {
    let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
    let text = |key: &str| body[key].as_str().map(str::to_owned);
    let (Some(repo), Some(pull), Some(login), Some(content)) = (
        text("repo"),
        body["pull"].as_u64(),
        text("login"),
        text("content"),
    ) else {
        return Response::empty(400);
    };
    let reaction = Reaction {
        repo,
        pull,
        login,
        content,
    };
    match request.method.as_str() {
        "POST" => set(state, reaction, true),
        "DELETE" => set(state, reaction, false),
        _ => return Response::empty(405),
    }
    Response::empty(204)
}
