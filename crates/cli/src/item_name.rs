//! Items as agents and the operator know them. On GitHub an item is named by
//! its pull request, as in `app#84`, not by its ID (`app#3`); commands take
//! either and ask the hub which item it is, then address the hub by ID.

use api::item::Item;

use crate::client::{Client, segment};
use crate::output::CliError;

/// The item `name` names, or whose ID it is.
pub(crate) fn find(client: &Client, name: &str) -> Result<Item, CliError> {
    client.get(&format!("/api/names/{}", segment(name)))
}
