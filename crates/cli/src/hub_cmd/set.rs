use api::settings::{SetSetting, Settings};
use hub::Home;

use crate::client::{self, Client};
use crate::output::{CliError, Outcome};

pub(super) fn run(home: &Home, key: String, value: String) -> Result<Outcome, CliError> {
    let client = Client::connect(home)?;
    let path = format!("/api/hub/settings/{}", client::segment(&key));
    let settings: Settings = client.put(&path, &SetSetting { value })?;
    let text = match key.as_str() {
        "listen" => listen(&settings),
        _ => format!("attachment-ttl {}", settings.attachment_ttl),
    };
    Ok(Outcome::done(&settings, text))
}

fn listen(settings: &Settings) -> String {
    let tailnet = settings.tailnet.as_ref();
    match (
        tailnet.and_then(|t| t.url.as_deref()),
        tailnet.and_then(|t| t.problem.as_deref()),
    ) {
        (Some(url), _) => {
            format!("listen tailnet: listening at {url}\non another machine: mg connect {url}")
        }
        (None, Some(problem)) => {
            format!("listen tailnet: not listening yet, retrying: {problem}")
        }
        (None, None) => format!("listen {}", settings.listen),
    }
}
