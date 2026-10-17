//! External review: comments that bots and teammates leave on a github
//! item's pull request. The hub polls them, shows them as they come, and
//! hands them to the item's agent in batches once the reviewers have gone
//! quiet. Each goes only once the operator votes it up, so the agent isn't
//! sent after every nit a reviewer raises; the operator can agree with the
//! agent's rejection of one or override it.

mod due;
mod error;
mod gone;
mod ingest;
mod on_read;
mod rejection;
mod store;
mod vote;

pub use due::{Timing, due, release, release_due};
pub use error::ExternalError;
pub use gone::{GithubKey, confirm_gone, held_keys};
pub use ingest::{Ingested, fetch, ingest, pull_of};
pub use on_read::release_on_read;
pub use rejection::{agree, override_rejection};
pub use store::{ExternalStore, Held, NewExternal};
pub use vote::vote;

use crate::github::PullCommentKind;
use crate::review::{Comment, Reply};

/// Where an external comment came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct External {
    pub kind: PullCommentKind,
    /// GitHub's, unique within the kind.
    pub github_id: u64,
    pub login: String,
    pub bot: bool,
    /// Its page on GitHub.
    pub url: String,
    /// The file an inline comment is on, kept when its lines couldn't be
    /// placed on a revision.
    pub path: Option<String>,
    /// Waiting for the next batch to carry it to the agent.
    pub pending: bool,
    /// The operator's vote, none until they cast it. Only a comment voted up
    /// reaches the agent.
    pub vote: Option<Vote>,
    /// The agent's rejection the operator agreed with. It stops counting once
    /// the thread has a later reply.
    pub agreed_reply: Option<u64>,
}

/// The operator's say on whether a comment is worth the agent's time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vote {
    /// Goes to the agent with the next batch.
    Up,
    /// Never goes to the agent.
    Down,
}

impl Vote {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            _ => None,
        }
    }
}

impl External {
    /// Held and not voted on yet: the batch waits for the operator.
    pub fn awaiting_vote(&self) -> bool {
        self.vote.is_none() && self.pending
    }

    /// Voted down: the agent never gets it, so it's no thread of the agent's.
    pub fn dismissed(&self) -> bool {
        self.vote == Some(Vote::Down)
    }
}

/// An `external.comments` event: comments that came while the agent wasn't
/// told yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    /// The item's newest revision when the batch went out.
    pub revision: u32,
    /// Oldest first.
    pub comments: Vec<Delivered>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivered {
    /// With its body as the batch carried it.
    pub comment: Comment,
    /// An earlier batch carried it with another body.
    pub edited: bool,
}

/// A `comment.override` event: the operator wants the change the agent
/// rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    pub comment: Comment,
    pub reply: Reply,
}

/// GitHub spells a GitHub App's login with a `[bot]` suffix that the
/// operator needn't type, and logins ignore case.
pub fn same_login(a: &str, b: &str) -> bool {
    let bare = |login: &str| login.strip_suffix("[bot]").unwrap_or(login).to_owned();
    bare(a).eq_ignore_ascii_case(&bare(b))
}

#[cfg(test)]
#[path = "external_tests.rs"]
mod tests;
