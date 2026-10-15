//! `mg hub devices | allow | deny`: which tailnet devices may reach the hub.

use api::device::{Access, Decided, Devices, SetAccess, Standing};
use hub::Home;

use crate::client::{self, Client};
use crate::output::{CliError, Outcome};

pub(super) fn list(home: &Home) -> Result<Outcome, CliError> {
    let client = Client::connect(home)?;
    let devices: Devices = client.get("/api/hub/devices")?;
    let width = devices
        .devices
        .iter()
        .map(|d| d.name.len())
        .max()
        .unwrap_or(0);
    let lines: Vec<_> = devices
        .devices
        .iter()
        .map(|d| {
            let standing = match d.standing {
                Standing::Owner => "owner",
                Standing::Allowed => "allowed",
                Standing::Denied => "denied",
                Standing::Asked => "asked",
                Standing::NotAllowed => "-",
            };
            let gone = if d.on_tailnet {
                ""
            } else {
                "  (left the tailnet)"
            };
            format!("{standing:8} {:width$}  {}{gone}", d.name, d.owner)
        })
        .collect();
    let text = match lines.is_empty() {
        true => "no devices on the tailnet".to_owned(),
        false => lines.join("\n"),
    };
    Ok(Outcome::done(&devices, text))
}

pub(super) fn decide(home: &Home, device: String, access: Access) -> Result<Outcome, CliError> {
    let client = Client::connect(home)?;
    let path = format!("/api/hub/devices/{}", client::segment(&device));
    let decided: Decided = client.put(&path, &SetAccess { access })?;
    let verb = match access {
        Access::Allow => "allowed",
        Access::Deny => "denied",
    };
    let text = format!("{verb} {} ({})", decided.name, decided.owner);
    Ok(Outcome::done(&decided, text))
}
