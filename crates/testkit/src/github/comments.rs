//! Comments on pull requests: the three lists the hub polls, with GitHub's
//! ETags and pages, and `/_fake/comments` for a process that can't call
//! [`FakeGithub::comment`](super::FakeGithub::comment) to post one.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::server::{Request, Response};
use super::{Shared, State};

/// A comment as the fake keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub repo: String,
    pub pull: u64,
    pub id: u64,
    pub kind: CommentKind,
    pub login: String,
    pub bot: bool,
    pub body: String,
    /// Where an inline comment is; the branch's head when it was posted
    /// fills in its commit.
    pub inline: Option<Inline>,
    /// When it was posted, in Unix seconds: GitHub dates comments to the
    /// second.
    pub created_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    /// On lines of the diff.
    Inline,
    /// On the conversation.
    Issue,
    /// A review's body, submitted.
    Review,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inline {
    pub path: String,
    /// The file before the change.
    pub old_side: bool,
    pub start_line: Option<u32>,
    pub line: u32,
    pub commit: String,
}

/// A comment to post: [`FakeGithub::comment`](super::FakeGithub::comment)
/// gives it its ID and commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewComment {
    pub kind: CommentKind,
    pub login: String,
    pub bot: bool,
    pub body: String,
    /// For [`CommentKind::Inline`]: path, side, lines.
    pub lines: Option<(String, bool, Option<u32>, u32)>,
}

impl NewComment {
    /// On the conversation, by a person unless `login` ends in `[bot]`.
    pub fn issue(login: &str, body: &str) -> Self {
        Self {
            kind: CommentKind::Issue,
            login: login.into(),
            bot: login.ends_with("[bot]"),
            body: body.into(),
            lines: None,
        }
    }

    /// A review submitted with `body`.
    pub fn review(login: &str, body: &str) -> Self {
        Self {
            kind: CommentKind::Review,
            ..Self::issue(login, body)
        }
    }

    /// On lines `start..=line` of `path` after the change.
    pub fn inline(login: &str, body: &str, path: &str, start: Option<u32>, line: u32) -> Self {
        Self {
            kind: CommentKind::Inline,
            lines: Some((path.into(), false, start, line)),
            ..Self::issue(login, body)
        }
    }
}

/// Adds `comment` to `pull`, inline ones at `head`; its ID.
pub(super) fn add(
    state: &mut State,
    repo: &str,
    pull: u64,
    head: Option<String>,
    comment: NewComment,
) -> u64 {
    state.last_comment += 1;
    let id = state.last_comment;
    let inline = match (comment.kind, comment.lines) {
        (CommentKind::Inline, Some((path, old_side, start_line, line))) => Some(Inline {
            path,
            old_side,
            start_line,
            line,
            commit: head.unwrap_or_default(),
        }),
        (CommentKind::Inline, None) => panic!("an inline comment needs lines"),
        _ => None,
    };
    state.comments.push(Comment {
        repo: repo.into(),
        pull,
        id,
        kind: comment.kind,
        login: comment.login,
        bot: comment.bot,
        body: comment.body,
        inline,
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    });
    id
}

/// `GET .../pulls/{n}/comments`, `.../issues/{n}/comments` or
/// `.../pulls/{n}/reviews`, a page at a time, with an ETag.
pub(super) fn list(
    shared: &Shared,
    state: &mut State,
    request: &Request,
    repo: &str,
    number: &str,
    kind: CommentKind,
    head: Option<String>,
) -> Response {
    let number = number.parse::<u64>().unwrap_or(0);
    if !state
        .pulls
        .iter()
        .any(|p| p.repo == repo && p.number == number)
    {
        return Response::json(404, &json!({ "message": "Not Found" }));
    }
    let all: Vec<Value> = state
        .comments
        .iter()
        .filter(|c| c.repo == repo && c.pull == number && c.kind == kind)
        .filter(|c| !state.unlisted.contains(&c.id))
        .map(|c| to_json(&shared.web, c, head.as_deref()))
        .collect();
    let per_page = request
        .param("per_page")
        .and_then(|n| n.parse().ok())
        .unwrap_or(30usize)
        .min(state.page_size)
        .max(1);
    let page = request
        .param("page")
        .and_then(|n| n.parse().ok())
        .unwrap_or(1usize)
        .max(1);
    let items: Vec<Value> = all
        .iter()
        .skip((page - 1) * per_page)
        .take(per_page)
        .cloned()
        .collect();
    let body = Value::Array(items);
    let etag = format!("W/\"{:x}\"", digest(&body.to_string()));
    if request.header("if-none-match") == Some(etag.as_str()) {
        state.not_modified += 1;
        return Response::empty(304);
    }
    let mut response = Response::json(200, &body);
    response.headers.push(("ETag".into(), etag));
    if page * per_page < all.len() {
        let next = format!(
            "{}/api{}?per_page={per_page}&page={}",
            shared.web,
            request.path.trim_start_matches("/api"),
            page + 1
        );
        response
            .headers
            .push(("Link".into(), format!("<{next}>; rel=\"next\"")));
    }
    response
}

/// `GET .../pulls/comments/{id}`, `.../issues/comments/{id}` or
/// `.../pulls/{n}/reviews/{id}`: one comment, listed or not.
pub(super) fn one(
    shared: &Shared,
    state: &State,
    repo: &str,
    pull: Option<&str>,
    kind: CommentKind,
    id: &str,
) -> Response {
    let pull = pull.map(|n| n.parse::<u64>().unwrap_or(0));
    let id = id.parse::<u64>().unwrap_or(0);
    let found = state.comments.iter().find(|c| {
        c.repo == repo && c.kind == kind && c.id == id && pull.is_none_or(|n| n == c.pull)
    });
    match found {
        Some(comment) => {
            let head = shared.pull_head(state, repo, comment.pull);
            Response::json(200, &to_json(&shared.web, comment, head.as_deref()))
        }
        None => Response::json(404, &json!({ "message": "Not Found" })),
    }
}

fn to_json(web: &str, comment: &Comment, head: Option<&str>) -> Value {
    let page = format!("{web}/{}/pull/{}", comment.repo, comment.pull);
    let user = json!({
        "login": comment.login,
        "type": if comment.bot { "Bot" } else { "User" },
    });
    let created = stamp(comment.created_at);
    match (&comment.kind, &comment.inline) {
        (CommentKind::Inline, Some(inline)) => {
            // GitHub carries the lines to the head while it's the commit
            // they were written on, and drops them once a push outdates them.
            let current = head == Some(inline.commit.as_str());
            json!({
                "id": comment.id,
                "user": user,
                "html_url": format!("{page}#discussion_r{}", comment.id),
                "body": comment.body,
                "path": inline.path,
                "side": if inline.old_side { "LEFT" } else { "RIGHT" },
                "original_commit_id": inline.commit,
                "original_start_line": inline.start_line,
                "original_line": inline.line,
                "commit_id": head.unwrap_or(&inline.commit),
                "start_line": if current { inline.start_line } else { None },
                "line": current.then_some(inline.line),
                "created_at": created,
                "updated_at": created,
            })
        }
        (CommentKind::Review, _) => json!({
            "id": comment.id,
            "user": user,
            "html_url": format!("{page}#pullrequestreview-{}", comment.id),
            "body": comment.body,
            "state": "COMMENTED",
            "submitted_at": created,
        }),
        _ => json!({
            "id": comment.id,
            "user": user,
            "html_url": format!("{page}#issuecomment-{}", comment.id),
            "body": comment.body,
            "created_at": created,
            "updated_at": created,
        }),
    }
}

/// `POST /_fake/comments` with `{repo, pull, kind, login, bot, body}`, and
/// `path`, `line` and optionally `start_line` and `side: "LEFT"` for an
/// inline one; `PATCH /_fake/comments/{id}` with `{body}` edits one.
pub(super) fn control(shared: &Shared, request: &Request) -> Response {
    let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
    let text = |key: &str| body[key].as_str().unwrap_or_default().to_owned();
    let segments: Vec<&str> = request.path.trim_start_matches('/').split('/').collect();
    match (request.method.as_str(), segments.as_slice()) {
        ("POST", ["_fake", "comments"]) => {
            let kind = match body["kind"].as_str() {
                Some("inline") => CommentKind::Inline,
                Some("review") => CommentKind::Review,
                _ => CommentKind::Issue,
            };
            let line = |key: &str| body[key].as_u64().map(|n| n as u32);
            let comment = NewComment {
                kind,
                login: text("login"),
                bot: body["bot"] == true,
                body: text("body"),
                lines: line("line")
                    .map(|l| (text("path"), body["side"] == "LEFT", line("start_line"), l)),
            };
            let repo = text("repo");
            let pull = body["pull"].as_u64().unwrap_or(0);
            let mut state = shared.state();
            let head = shared.pull_head(&state, &repo, pull);
            let id = add(&mut state, &repo, pull, head, comment);
            Response::json(201, &json!({ "id": id }))
        }
        ("PATCH", ["_fake", "comments", id]) => {
            let id: u64 = id.parse().unwrap_or(0);
            let edited = edit(&mut shared.state(), id, &text("body"));
            match edited {
                true => Response::json(200, &json!({ "id": id })),
                false => Response::empty(404),
            }
        }
        _ => Response::empty(404),
    }
}

pub(super) fn edit(state: &mut State, id: u64, body: &str) -> bool {
    match state.comments.iter_mut().find(|c| c.id == id) {
        Some(comment) => {
            comment.body = body.into();
            true
        }
        None => false,
    }
}

fn digest(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// `secs` since the epoch as GitHub writes a time: `2026-01-02T03:04:05Z`.
fn stamp(secs: u64) -> String {
    // Howard Hinnant's civil_from_days, on days since 1970-01-01.
    let z = secs / 86_400 + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    let time = secs % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time / 3_600,
        time / 60 % 60,
        time % 60
    )
}
