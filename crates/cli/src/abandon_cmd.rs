//! `mg abandon`: the agent closes its item without merging it, as when the
//! work moved elsewhere. The operator reads why in the UI.

use api::feedback::Source;
use api::item::{Abandon, Item};

use crate::client::{self, Client};
use crate::output::{CliError, Outcome};

pub fn run(item: String, reason: Option<String>) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    let request = Abandon {
        source: Source::Agent,
        reason,
    };
    let path = format!("/api/items/{}/abandon", client::segment(&item));
    let item = match client.post::<Item>(&path, &request)? {
        client::Reply::Done(item) => item,
        client::Reply::Decide(decision) => return Ok(Outcome::NeedsDecision(decision)),
    };
    let text = format!("abandoned {}", item.id);
    Ok(Outcome::done(&item, text))
}
