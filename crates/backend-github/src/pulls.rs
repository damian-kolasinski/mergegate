//! Pull requests.

use mergegate_core::github::{GithubError, GithubRepo, Pull};
use serde_json::{Value, json};
use ureq::http::Method;

use crate::api::{Api, Reply, encode};

pub(crate) fn find(
    api: &Api,
    repo: &GithubRepo,
    branch: &str,
    target: &str,
) -> Result<Option<Pull>, GithubError> {
    let head = encode(&format!("{}:{branch}", repo.owner()));
    let base = encode(target);
    let reply = api.get(&format!(
        "/repos/{repo}/pulls?head={head}&base={base}&state=open&per_page=1"
    ))?;
    if !reply.ok() {
        return Err(not_found_is_access(repo, &reply));
    }
    reply
        .body
        .as_array()
        .and_then(|pulls| pulls.first())
        .map(pull)
        .transpose()
}

/// Ready for review: the item's review happens in mergegate.
pub(crate) fn open(
    api: &Api,
    repo: &GithubRepo,
    branch: &str,
    target: &str,
    title: &str,
    description: &str,
) -> Result<Pull, GithubError> {
    let body = json!({
        "title": title,
        "head": branch,
        "base": target,
        "body": description,
        "draft": false,
    });
    let reply = api.send(Method::POST, &format!("/repos/{repo}/pulls"), body)?;
    if !reply.ok() {
        return Err(not_found_is_access(repo, &reply));
    }
    pull(&reply.body)
}

pub(crate) fn body(api: &Api, repo: &GithubRepo, number: u64) -> Result<String, GithubError> {
    let reply = api.get(&format!("/repos/{repo}/pulls/{number}"))?;
    if !reply.ok() {
        return Err(not_found_is_access(repo, &reply));
    }
    // GitHub sends `null` for a pull request without a description.
    Ok(reply.body["body"].as_str().unwrap_or_default().to_owned())
}

pub(crate) fn edit(
    api: &Api,
    repo: &GithubRepo,
    number: u64,
    title: &str,
    description: Option<&str>,
) -> Result<(), GithubError> {
    let mut body = json!({ "title": title });
    if let Some(description) = description {
        body["body"] = description.into();
    }
    update(api, repo, number, body)
}

pub(crate) fn retarget(
    api: &Api,
    repo: &GithubRepo,
    number: u64,
    base: &str,
) -> Result<(), GithubError> {
    update(api, repo, number, json!({ "base": base }))
}

pub(crate) fn close(api: &Api, repo: &GithubRepo, number: u64) -> Result<(), GithubError> {
    update(api, repo, number, json!({ "state": "closed" }))
}

/// GitHub merges only while the pull request's head is `head`, the
/// revision approved. One it already merged at `head` reads as merged, so a
/// retry after the hub failed to record the merge finds its commit.
pub(crate) fn squash_merge(
    api: &Api,
    repo: &GithubRepo,
    number: u64,
    title: &str,
    message: &str,
    head: &str,
) -> Result<String, GithubError> {
    let body = json!({
        "merge_method": "squash",
        "commit_title": title,
        "commit_message": message,
        "sha": head,
    });
    let reply = api.send(
        Method::PUT,
        &format!("/repos/{repo}/pulls/{number}/merge"),
        body,
    )?;
    if reply.ok() {
        return sha(&reply.body["sha"]);
    }
    if matches!(reply.status, 405 | 409 | 422)
        && let Some(commit) = merged(api, repo, number, head)?
    {
        return Ok(commit);
    }
    Err(not_found_is_access(repo, &reply))
}

pub(crate) fn merged(
    api: &Api,
    repo: &GithubRepo,
    number: u64,
    head: &str,
) -> Result<Option<String>, GithubError> {
    let reply = api.get(&format!("/repos/{repo}/pulls/{number}"))?;
    if !reply.ok() {
        return Err(not_found_is_access(repo, &reply));
    }
    if reply.body["merged"] == true && reply.body["head"]["sha"] == head {
        return sha(&reply.body["merge_commit_sha"]).map(Some);
    }
    Ok(None)
}

fn update(api: &Api, repo: &GithubRepo, number: u64, body: Value) -> Result<(), GithubError> {
    let reply = api.send(
        Method::PATCH,
        &format!("/repos/{repo}/pulls/{number}"),
        body,
    )?;
    if !reply.ok() {
        return Err(not_found_is_access(repo, &reply));
    }
    Ok(())
}

fn not_found_is_access(repo: &GithubRepo, reply: &Reply) -> GithubError {
    match reply.status {
        404 => GithubError::NoAccess(repo.clone()),
        _ => reply.error(),
    }
}

fn pull(body: &Value) -> Result<Pull, GithubError> {
    let number = body["number"].as_u64();
    let url = body["html_url"].as_str();
    match (number, url) {
        (Some(number), Some(url)) => Ok(Pull {
            number,
            url: url.into(),
        }),
        _ => Err(GithubError::Failed(format!(
            "GitHub sent a pull request without its number or URL: {body}"
        ))),
    }
}

fn sha(value: &Value) -> Result<String, GithubError> {
    value
        .as_str()
        .map(Into::into)
        .ok_or_else(|| GithubError::Failed("GitHub sent a merge without its commit".into()))
}
