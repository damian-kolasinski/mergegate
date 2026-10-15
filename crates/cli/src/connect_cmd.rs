//! `mg connect`: points this machine's `mg` at a hub on another machine,
//! over the tailnet, or back at its own.

use std::net::Ipv6Addr;

use api::device::{Caller, Trust, Via, WhoAmI};
use hub::Connection;
use serde::Serialize;

use crate::client::{self, Client};
use crate::output::{CliError, Outcome};

#[derive(Serialize)]
struct Connected {
    url: String,
    device: Caller,
}

#[derive(Serialize)]
struct Disconnected {
    /// Whether there was a connection to remove.
    disconnected: bool,
}

pub fn run(hub: Option<String>, local: bool) -> Result<Outcome, CliError> {
    let home = crate::home()?;
    let Some(hub) = hub.filter(|_| !local) else {
        let disconnected = home
            .remove_connection()
            .map_err(|e| CliError::new("io_error", format!("removing connect.json: {e}")))?;
        let text = match disconnected {
            true => "mg uses the hub on this machine again",
            false => "mg already uses the hub on this machine",
        };
        return Ok(Outcome::done(&Disconnected { disconnected }, text));
    };
    let url = url(&hub)?;
    // Saved only once the hub lets this machine in, so a typo or a refusal
    // leaves mg as it was.
    let client = Client::to(&client::reach(&url)?)?;
    let whoami: WhoAmI = client.get("/api/whoami")?;
    let device = match (whoami.via, whoami.device) {
        (Via::Tailnet, Some(device)) => device,
        _ => {
            return Err(CliError::new(
                "local_hub",
                format!("{url} is this machine's hub over loopback, which mg finds on its own"),
            )
            .hint("mg connect --local"));
        }
    };
    home.write_connection(&Connection { url: url.clone() })
        .map_err(|e| CliError::new("io_error", format!("writing connect.json: {e}")))?;
    let trust = match device.trust {
        Trust::Owner => "the hub owner's device",
        Trust::Allowed => "allowed by the operator",
    };
    let text = format!("connected to the hub at {url} as {} ({trust})", device.name);
    Ok(Outcome::done(&Connected { url, device }, text))
}

/// `mac-mini`, `mac-mini:7801`, `100.64.0.1` or `http://mac-mini:7800/` as
/// the URL mg sends requests to. The tailnet encrypts, so the hub speaks
/// plain HTTP there as on loopback.
fn url(hub: &str) -> Result<String, CliError> {
    let invalid = |why: &str| {
        CliError::new("invalid_hub", format!("{hub:?} {why}"))
            .hint("mg connect HOST, as in mg connect mac-mini")
    };
    let hub = hub.trim().trim_end_matches('/');
    let address = match hub.split_once("://") {
        Some(("http", address)) => address,
        Some(_) => return Err(invalid("isn't an http:// URL; the hub serves plain HTTP")),
        None => hub,
    };
    if address.is_empty() || address.contains(['/', '?', '#', '@']) {
        return Err(invalid("isn't a host, host:port or hub URL"));
    }
    let port = hub::DEFAULT_PORT;
    let address = match address.strip_prefix('[') {
        Some(bracketed) if bracketed.contains("]:") => address.to_owned(),
        Some(_) => format!("{address}:{port}"),
        // An IPv6 address without brackets can't carry a port.
        None if address.matches(':').count() > 1 => format!("[{address}]:{port}"),
        None if address.contains(':') => address.to_owned(),
        None => format!("{address}:{port}"),
    };
    // Every form above ends in a port.
    let (host, port) = address.rsplit_once(':').unwrap_or((&address, ""));
    // Left to the request, a malformed host would read as a hub that's
    // unreachable, which is worth retrying; it never is.
    // Brackets, ours or the operator's, hold only an IPv6 address.
    let valid_host = match host.strip_prefix('[') {
        Some(bracketed) => bracketed
            .strip_suffix(']')
            .is_some_and(|ip| ip.parse::<Ipv6Addr>().is_ok()),
        None => {
            !host.is_empty()
                && !host.contains(['[', ']', ':'])
                && !host.chars().any(|c| c.is_whitespace() || c.is_control())
        }
    };
    if !valid_host {
        return Err(invalid("isn't a host, host:port or hub URL"));
    }
    if port.parse::<u16>().is_err() {
        return Err(invalid("has no valid port"));
    }
    Ok(format!("http://{address}"))
}

#[cfg(test)]
#[path = "connect_cmd_tests.rs"]
mod tests;
