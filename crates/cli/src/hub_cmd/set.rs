use api::settings::{SetSetting, Settings};
use hub::Home;

use crate::client::{self, Client};
use crate::output::{CliError, Outcome};

pub(super) fn run(home: &Home, key: String, value: String) -> Result<Outcome, CliError> {
    let client = Client::connect(home)?;
    let path = format!("/api/hub/settings/{}", client::segment(&key));
    let settings: Settings = client.put(&path, &SetSetting { value })?;
    let text = format!("attachment-ttl {}", settings.attachment_ttl);
    Ok(Outcome::done(&settings, text))
}
