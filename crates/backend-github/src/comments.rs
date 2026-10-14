//! A pull request's comments, and who the operator is on GitHub. The hub
//! polls these every few seconds per item, so a list that fits one page is
//! asked for again with its ETag: GitHub answers 304 for free when nothing
//! changed.

use std::collections::HashMap;
use std::sync::Mutex;

use mergegate_core::github::{GithubError, GithubRepo, PullComment, PullCommentKind, PullLines};
use serde_json::Value;

use crate::api::{Api, Reply};

/// GitHub's largest page.
const PER_PAGE: u32 = 100;

#[derive(Default)]
pub(crate) struct Cache {
    viewer: Mutex<Option<String>>,
    /// A one-page list's ETag and items, by its path.
    lists: Mutex<HashMap<String, (String, Vec<Value>)>>,
}

/// Asked once: the hub signs in as the same account until `gh` says
/// otherwise, and a new login is the same operator.
pub(crate) fn viewer(api: &Api, cache: &Cache) -> Result<String, GithubError> {
    let mut viewer = cache.viewer.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(login) = viewer.as_ref() {
        return Ok(login.clone());
    }
    let reply = api.get("/user")?;
    if !reply.ok() {
        return Err(reply.error());
    }
    let login = reply.body["login"]
        .as_str()
        .ok_or_else(|| GithubError::Failed("GitHub sent a user without a login".into()))?
        .to_owned();
    *viewer = Some(login.clone());
    Ok(login)
}

pub(crate) fn of_pull(
    api: &Api,
    cache: &Cache,
    repo: &GithubRepo,
    number: u64,
    fresh: bool,
) -> Result<Vec<PullComment>, GithubError> {
    let mut comments = Vec::new();
    for kind in PullCommentKind::ALL {
        let list = match kind {
            PullCommentKind::ReviewComment => "pulls/{n}/comments",
            PullCommentKind::IssueComment => "issues/{n}/comments",
            PullCommentKind::Review => "pulls/{n}/reviews",
        }
        .replace("{n}", &number.to_string());
        let path = format!("/repos/{repo}/{list}?per_page={PER_PAGE}");
        for value in items(api, cache, &path, fresh)? {
            if let Some(comment) = comment(kind, &value)? {
                comments.push(comment);
            }
        }
    }
    Ok(comments)
}

/// Every item of the list at `path`, following its pages.
fn items(api: &Api, cache: &Cache, path: &str, fresh: bool) -> Result<Vec<Value>, GithubError> {
    let lists = || cache.lists.lock().unwrap_or_else(|e| e.into_inner());
    let known = lists().get(path).filter(|_| !fresh).cloned();
    let reply = api.get_if(path, known.as_ref().map(|(etag, _)| etag.as_str()))?;
    if reply.status == 304
        && let Some((_, items)) = known
    {
        return Ok(items);
    }
    let mut items = page(&reply)?;
    let mut lists = lists();
    match (&reply.next, reply.etag) {
        // A later page's change wouldn't show in the first's ETag.
        (Some(_), _) | (None, None) => {
            lists.remove(path);
        }
        (None, Some(etag)) => {
            lists.insert(path.to_owned(), (etag, items.clone()));
        }
    }
    drop(lists);
    let mut next = reply.next;
    while let Some(url) = next {
        let reply = api.get(&url)?;
        items.extend(page(&reply)?);
        next = reply.next;
    }
    Ok(items)
}

fn page(reply: &Reply) -> Result<Vec<Value>, GithubError> {
    if !reply.ok() {
        return Err(reply.error());
    }
    reply
        .body
        .as_array()
        .cloned()
        .ok_or_else(|| GithubError::Failed("GitHub sent a list that isn't one".into()))
}

/// None for a review still pending: only its author sees it.
fn comment(kind: PullCommentKind, value: &Value) -> Result<Option<PullComment>, GithubError> {
    if kind == PullCommentKind::Review && value["state"] == "PENDING" {
        return Ok(None);
    }
    let malformed = |field| GithubError::Failed(format!("GitHub sent a comment without {field}"));
    let text = |field: &'static str| {
        value[field]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| malformed(field))
    };
    let line = |field: &str| value[field].as_u64().map(|n| n as u32);
    let lines = match kind {
        PullCommentKind::ReviewComment => Some(PullLines {
            path: text("path")?,
            old_side: value["side"] == "LEFT",
            original_commit: text("original_commit_id")?,
            original_start_line: line("original_start_line"),
            original_line: line("original_line"),
            commit: text("commit_id")?,
            start_line: line("start_line"),
            line: line("line"),
        }),
        PullCommentKind::IssueComment | PullCommentKind::Review => None,
    };
    Ok(Some(PullComment {
        kind,
        id: value["id"].as_u64().ok_or_else(|| malformed("id"))?,
        // A deleted account's comments have no user.
        login: value["user"]["login"]
            .as_str()
            .unwrap_or("ghost")
            .to_owned(),
        bot: value["user"]["type"] == "Bot",
        url: text("html_url")?,
        // A review submitted without a body has it null.
        body: value["body"].as_str().unwrap_or_default().to_owned(),
        lines,
    }))
}

#[cfg(test)]
#[path = "comments_tests.rs"]
mod tests;
