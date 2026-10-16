//! `mg sync`: reads the item's pull request comments from GitHub now, and
//! hands what the hub holds to the agent at once. The hub polls on its own;
//! this is for when it missed something.

use api::review::Synced;

use crate::client::{self, Client};
use crate::item_name;
use crate::output::{CliError, Outcome};

pub fn run(item: String) -> Result<Outcome, CliError> {
    let client = Client::connect(&crate::home()?)?;
    let found = item_name::find(&client, &item)?;
    let path = format!("/api/items/{}/sync", client::segment(&found.id));
    let item = found.name;
    let synced = match client.post::<Synced>(&path, &serde_json::json!({}))? {
        client::Reply::Done(synced) => synced,
        client::Reply::Decide(decision) => return Ok(Outcome::NeedsDecision(decision)),
    };
    let mut text = format!(
        "synced {item} with GitHub: {} new, {} edited",
        synced.new, synced.edited
    );
    match synced.released {
        0 => {}
        1 => text.push_str(&format!(
            "\n1 comment is waiting for you: mg feedback {item}"
        )),
        n => text.push_str(&format!(
            "\n{n} comments are waiting for you: mg feedback {item}"
        )),
    }
    Ok(Outcome::done(&synced, text))
}
