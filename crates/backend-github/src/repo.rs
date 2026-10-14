//! The repository itself: reaching it with git, the operator's access to it,
//! and its branches.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use mergegate_core::git::{Remote, Secret};
use mergegate_core::github::{GithubError, GithubRepo};

use crate::api::{Api, encode};

/// HTTPS, whatever protocol the operator's own remotes use: the token signs
/// git in, so no SSH key or credential helper is needed. Read afresh, since
/// git can't ask for a new one the way the API calls do after a 401.
pub(crate) fn remote(api: &Api, web: &str, repo: &GithubRepo) -> Result<Remote, GithubError> {
    api.token().forget();
    let token = api.token().get()?;
    let basic = STANDARD.encode(format!("x-access-token:{}", token.expose()));
    Ok(Remote {
        url: format!("{web}/{repo}.git"),
        auth: Secret::new(format!("Authorization: Basic {basic}")),
    })
}

/// GitHub answers 404 for a private repository the account can't see, so
/// that and a missing repository read the same.
pub(crate) fn check_access(api: &Api, repo: &GithubRepo) -> Result<(), GithubError> {
    let reply = api.get(&format!("/repos/{repo}"))?;
    match reply.status {
        200 if reply.body["permissions"]["push"] == true => Ok(()),
        200 | 403 | 404 => Err(GithubError::NoAccess(repo.clone())),
        _ => Err(reply.error()),
    }
}

pub(crate) fn delete_branch(api: &Api, repo: &GithubRepo, branch: &str) -> Result<(), GithubError> {
    let reply = api.delete(&format!("/repos/{repo}/git/refs/heads/{}", encode(branch)))?;
    match reply.status {
        204 | 404 => Ok(()),
        422 if reply.body["message"] == "Reference does not exist" => Ok(()),
        _ => Err(reply.error()),
    }
}
