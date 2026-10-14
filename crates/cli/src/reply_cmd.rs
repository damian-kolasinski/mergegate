//! `mg reply`: the agent answers a comment from the operator's review. The
//! comment's latest status is the one that counts.

use clap::ValueEnum;

use api::review::{Reply, ReplyStatus, WriteReply};

use crate::client::{self, Client};
use crate::output::{CliError, Outcome};

#[derive(Clone, Copy, ValueEnum)]
pub enum Status {
    /// The next revision has the change
    Addressed,
    /// Won't change it; --body says why
    Rejected,
    /// The operator has to answer first; --body asks
    Question,
}

pub fn run(comment: String, status: Status, body: Option<String>) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    let request = WriteReply {
        status: match status {
            Status::Addressed => ReplyStatus::Addressed,
            Status::Rejected => ReplyStatus::Rejected,
            Status::Question => ReplyStatus::Question,
        },
        body: body.unwrap_or_default(),
    };
    let path = format!("/api/comments/{}/replies", client::segment(&comment));
    let reply = match client.post::<Reply>(&path, &request)? {
        client::Reply::Done(reply) => reply,
        client::Reply::Decide(decision) => return Ok(Outcome::NeedsDecision(decision)),
    };
    let status = match status {
        Status::Addressed => "addressed",
        Status::Rejected => "rejected",
        Status::Question => "a question",
    };
    let text = format!("marked {} {status}", reply.comment);
    Ok(Outcome::done(&reply, text))
}
