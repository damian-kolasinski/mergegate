//! What review bots say about a pull request through their reactions on its
//! description. Codex reacts 👀 while a review of it runs and 👍 once every
//! review finished without findings; findings come as comments instead,
//! which reach the item as external comments.

use std::collections::BTreeMap;

use super::{PullReaction, Reaction};

/// A bot's say on a pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotReview {
    pub login: String,
    /// What the operator knows the bot as, as in `Codex`.
    pub name: String,
    pub state: BotReviewState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotReviewState {
    /// A review is running.
    Reviewing,
    /// Its reviews finished without findings.
    Approved,
}

/// Bots whose logins don't say what they're known as; the rest go by their
/// login without `[bot]`.
const NAMES: &[(&str, &str)] = &[("chatgpt-codex-connector[bot]", "Codex")];

/// One per bot that reacted 👀 or 👍, by login, on a pull request whose
/// newest commits were pushed at `pushed`, in RFC 3339 UTC. 👀 wins: a bot
/// that keeps its 👍 from an earlier review while it reviews new commits
/// hasn't approved those yet.
///
/// A 👍 from before `pushed` approved older commits, and stays when a
/// review of the new ones finds something, so it says nothing. Times
/// compare to the second, as GitHub has them; a 👍 in the same second as
/// the push counts, since a review takes far longer than that.
pub fn bot_reviews(reactions: &[PullReaction], pushed: &str) -> Vec<BotReview> {
    let mut states = BTreeMap::new();
    for reaction in reactions.iter().filter(|r| r.bot) {
        let state = match reaction.reaction {
            Reaction::Eyes => BotReviewState::Reviewing,
            Reaction::ThumbsUp if seconds(&reaction.at) >= seconds(pushed) => {
                BotReviewState::Approved
            }
            Reaction::ThumbsUp | Reaction::Other => continue,
        };
        let known = states.entry(reaction.login.as_str()).or_insert(state);
        if state == BotReviewState::Reviewing {
            *known = state;
        }
    }
    states
        .into_iter()
        .map(|(login, state)| BotReview {
            login: login.to_owned(),
            name: name(login),
            state,
        })
        .collect()
}

/// `YYYY-MM-DDTHH:MM:SS`, which orders UTC times as text whatever follows:
/// GitHub's `Z` or the hub's milliseconds.
fn seconds(time: &str) -> &str {
    time.get(..19).unwrap_or(time)
}

fn name(login: &str) -> String {
    let known = NAMES.iter().find(|(known, _)| *known == login);
    match known {
        Some((_, name)) => (*name).to_owned(),
        None => login.trim_end_matches("[bot]").to_owned(),
    }
}

#[cfg(test)]
#[path = "bot_review_tests.rs"]
mod tests;
