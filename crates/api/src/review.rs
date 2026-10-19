use serde::{Deserialize, Serialize};

/// `POST /api/items/{id}/reviews`, answered with [`Review`].
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitReview {
    /// The revision the review was written on.
    pub revision: u32,
    pub verdict: Verdict,
    #[serde(default)]
    pub summary: String,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Leaves the item waiting for review.
    Comment,
    /// Sends the item back to its agent.
    ChangesRequested,
    /// Merges the item. Only `POST /api/items/{id}/approve` sends it; the
    /// reviews route refuses it.
    Approved,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    pub id: u64,
    pub revision: u32,
    pub verdict: Verdict,
    pub summary: String,
    /// The drafts the review sent, oldest first.
    pub comments: Vec<Comment>,
    /// The operator's draft replies the review sent, oldest first.
    pub replies: Vec<Reply>,
    pub submitted_at: String,
}

/// A comment on lines `start_line..=line` of one side of a file, or, only
/// for one from GitHub, on the item as a whole: then `path`, `side`, the
/// lines and `quote` are all null.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    /// Hub-wide, as in `c_17`.
    pub id: String,
    /// The revision it was written on.
    pub revision: u32,
    pub path: Option<String>,
    pub side: Option<Side>,
    pub start_line: Option<u32>,
    pub line: Option<u32>,
    pub body: String,
    /// The last few of the commented lines.
    pub quote: Option<String>,
    /// Where a comment someone left on the item's pull request came from;
    /// absent for the operator's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub external: Option<ExternalComment>,
    /// Where it shows on the revision the conversation was read onto, when
    /// it's on lines of an earlier one; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub placement: Option<Placement>,
}

/// Where a comment from an earlier revision is on a later one.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Placement {
    /// On the same lines, unchanged, at these numbers. A draft placed so
    /// moves there when a review is submitted on that revision.
    Moved { start_line: u32, line: u32 },
    /// On the lines that replaced its lines, or hold what's left of them. A
    /// draft placed so holds the submit until it's re-placed or dropped.
    Changed { start_line: u32, line: u32 },
    /// Nowhere the revision's diff shows; as a draft, it holds the submit
    /// too.
    Outdated,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalComment {
    /// The GitHub login, as in `codex[bot]`.
    pub author: String,
    pub bot: bool,
    pub kind: ExternalKind,
    /// Its page on GitHub.
    pub url: String,
    /// The file an inline comment is on, also when its lines couldn't be
    /// placed on a revision.
    pub path: Option<String>,
    /// Not handed to the agent yet: the hub waits for the reviewers to go
    /// quiet, and for the operator's vote.
    pub pending: bool,
    /// A bot's summary of the pull request or of its review, for the
    /// operator to read: never held for a vote or sent to the agent.
    #[serde(default)]
    pub summary: bool,
    /// The operator's vote, none until they cast it: only a comment voted up
    /// reaches the agent.
    #[serde(default)]
    pub vote: Option<Vote>,
    /// The agent's rejection the operator agreed with.
    pub agreed_reply: Option<u64>,
}

/// The operator's say on a comment from GitHub before the agent sees it.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vote {
    /// Worth the agent's time: it goes with the next batch.
    Up,
    /// Not: the agent never gets it.
    Down,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalKind {
    /// On lines of the pull request's diff.
    ReviewComment,
    /// On the pull request's conversation.
    IssueComment,
    /// A review's body.
    Review,
}

/// The file before the change, or after it.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Old,
    New,
}

/// `POST /api/items/{id}/drafts`, answered with the draft's [`Comment`].
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteDraft {
    pub revision: u32,
    pub path: String,
    pub side: Side,
    pub start_line: u32,
    pub line: u32,
    pub body: String,
}

/// `PUT /api/items/{id}/drafts/{comment}/anchor`: re-places a draft on lines
/// of a revision, checked as a new draft's are. Answered with the
/// [`Comment`].
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveDraft {
    pub revision: u32,
    pub path: String,
    pub side: Side,
    pub start_line: u32,
    pub line: u32,
}

/// `PUT /api/items/{id}/drafts/{comment}`, answered with the [`Comment`].
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditDraft {
    pub body: String,
}

/// `POST /api/comments/{comment}/drafts` and
/// `PUT /api/comments/{comment}/drafts/{reply}`: the operator's draft reply on
/// a sent comment, which the next review sends. Answered with the [`Reply`].
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteDraftReply {
    pub body: String,
}

/// `GET /api/items/{id}/conversation`: the item's comments and replies on
/// every revision, which `GET /api/items/{id}` also carries.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    /// Sent with a review, oldest first.
    pub comments: Vec<Comment>,
    /// The operator's, not sent yet, oldest first.
    pub drafts: Vec<Comment>,
    /// The agent's and the sent ones of the operator, on every comment,
    /// oldest first.
    pub replies: Vec<Reply>,
    /// The operator's, not sent yet, oldest first.
    pub draft_replies: Vec<Reply>,
}

/// One entry of a sent comment's thread.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub id: u64,
    /// The comment it answers, as in `c_17`.
    pub comment: String,
    pub author: Author,
    /// The agent's; the latest one is the comment's. The operator's replies
    /// have none.
    pub status: Option<ReplyStatus>,
    pub body: String,
    /// The revision that addressed an `addressed` reply: the first one
    /// submitted after it.
    pub addressed_in: Option<u32>,
    /// The operator's override of the agent's rejection, sent at once.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[cfg_attr(feature = "ts", ts(as = "Option<bool>", optional))]
    pub overrides: bool,
    pub created_at: String,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Author {
    Agent,
    Operator,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyStatus {
    /// The next revision has the change.
    Addressed,
    /// The agent won't change it, and says why.
    Rejected,
    /// The agent needs the operator to answer first.
    Question,
}

/// `POST /api/comments/{comment}/override`: the operator wants the change
/// the agent rejected on a comment from GitHub. Answered with the [`Reply`],
/// which goes to the agent at once.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverrideRejection {
    /// Why, if the operator says.
    #[serde(default)]
    pub note: String,
}

/// `POST /api/comments/{comment}/vote`: the operator's vote on a
/// GitHub comment the hub holds from the agent, which they can change until a batch
/// carries it. Answered with the [`Comment`].
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CastVote {
    pub vote: Vote,
}

/// `POST /api/items/{id}/sync`: reads the pull request's comments again,
/// whatever GitHub cached, and hands what's held to the agent now.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Synced {
    /// Comments the hub hadn't seen.
    pub new: u32,
    /// Comments edited since the hub last saw them.
    pub edited: u32,
    /// How many comments went to the agent, in an `external.comments` event;
    /// one waits for the operator's vote.
    pub released: u32,
}

/// `POST /api/comments/{comment}/replies`: the agent answers a sent comment.
/// Answered with the [`Reply`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteReply {
    pub status: ReplyStatus,
    /// Required unless the status is `addressed`.
    #[serde(default)]
    pub body: String,
}
