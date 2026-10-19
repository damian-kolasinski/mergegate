//! Reactions on a pull request's description, polled alongside its comments
//! and cached the same way.

use mergegate_core::github::{GithubError, GithubRepo, PullReaction, Reaction};
use serde_json::Value;

use crate::api::Api;
use crate::comments::{self, Cache};

pub(crate) fn of_pull(
    api: &Api,
    cache: &Cache,
    repo: &GithubRepo,
    number: u64,
) -> Result<Vec<PullReaction>, GithubError> {
    let path = format!("/repos/{repo}/issues/{number}/reactions?per_page=100");
    let items = comments::items(api, cache, &path, false)?;
    Ok(items.iter().filter_map(reaction).collect())
}

fn reaction(value: &Value) -> Option<PullReaction> {
    let login = value["user"]["login"].as_str()?.to_owned();
    // GitHub types Codex's connector a User on its reactions, though its
    // comments say Bot, so the login's suffix counts too.
    let bot = value["user"]["type"] == "Bot" || login.ends_with("[bot]");
    let reaction = match value["content"].as_str()? {
        "+1" => Reaction::ThumbsUp,
        "eyes" => Reaction::Eyes,
        _ => Reaction::Other,
    };
    Some(PullReaction {
        login,
        bot,
        reaction,
        at: value["created_at"].as_str()?.to_owned(),
    })
}
