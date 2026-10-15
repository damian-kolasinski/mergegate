//! The JSON of `tailscale status --json` and `tailscale whois --json`, down
//! to the fields the hub reads.

use std::collections::HashMap;
use std::net::IpAddr;

use mergegate_core::device::{Device, DeviceId, Owner, Status, TailnetError};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StatusJson {
    backend_state: String,
    #[serde(rename = "Self")]
    me: Option<Node>,
    #[serde(default)]
    peer: Option<HashMap<String, Node>>,
    #[serde(default)]
    user: Option<HashMap<String, User>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Node {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "DNSName", default)]
    dns_name: String,
    #[serde(default)]
    host_name: String,
    #[serde(rename = "UserID", default)]
    user_id: u64,
    #[serde(rename = "TailscaleIPs", default)]
    tailscale_ips: Option<Vec<IpAddr>>,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct User {
    login_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WhoisJson {
    node: WhoisNode,
    user_profile: Profile,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WhoisNode {
    #[serde(rename = "StableID")]
    stable_id: String,
    name: String,
    /// As prefixes: `100.64.0.1/32`.
    #[serde(default)]
    addresses: Option<Vec<String>>,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Profile {
    #[serde(rename = "ID")]
    id: u64,
    login_name: String,
}

pub(crate) fn status(json: &[u8]) -> Result<Status, TailnetError> {
    let status: StatusJson = serde_json::from_slice(json).map_err(unreadable)?;
    // Logged out or stopped, the CLI still answers, but with no tailnet.
    if status.backend_state != "Running" {
        return Err(TailnetError::Unavailable(format!(
            "Tailscale isn't connected ({}); start it and log in",
            status.backend_state
        )));
    }
    let me = status
        .me
        .ok_or_else(|| TailnetError::Unavailable("tailscale status names no Self".into()))?;
    let users = status.user.unwrap_or_default();
    let device = |node: Node| {
        let login = users
            .get(&node.user_id.to_string())
            .map(|u| u.login_name.clone())
            .unwrap_or_else(|| node.user_id.to_string());
        let owner = owner(node.tags, node.user_id, login);
        let dns_name = match node.dns_name.as_str() {
            "" => node.host_name.to_lowercase(),
            name => name.trim_end_matches('.').to_owned(),
        };
        Device {
            id: DeviceId(node.id),
            name: short(&dns_name),
            dns_name,
            owner,
            ips: node.tailscale_ips.unwrap_or_default(),
        }
    };
    let mut peers: Vec<_> = status
        .peer
        .unwrap_or_default()
        .into_values()
        .map(device)
        .collect();
    peers.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Status {
        me: device(me),
        peers,
    })
}

pub(crate) fn whois(json: &[u8]) -> Result<Device, TailnetError> {
    let whois: WhoisJson = serde_json::from_slice(json).map_err(unreadable)?;
    let node = whois.node;
    let dns_name = node.name.trim_end_matches('.').to_owned();
    Ok(Device {
        id: DeviceId(node.stable_id),
        name: short(&dns_name),
        owner: owner(
            node.tags,
            whois.user_profile.id,
            whois.user_profile.login_name,
        ),
        ips: node
            .addresses
            .unwrap_or_default()
            .iter()
            .filter_map(|prefix| prefix.split('/').next()?.parse().ok())
            .collect(),
        dns_name,
    })
}

/// A tagged device belongs to its tags, whatever user Tailscale files it
/// under.
fn owner(tags: Option<Vec<String>>, id: u64, login: String) -> Owner {
    match tags {
        Some(tags) if !tags.is_empty() => Owner::Tags(tags),
        _ => Owner::User { id, login },
    }
}

fn short(dns_name: &str) -> String {
    dns_name.split('.').next().unwrap_or(dns_name).to_owned()
}

fn unreadable(e: serde_json::Error) -> TailnetError {
    TailnetError::Unavailable(format!("can't read tailscale's answer: {e}"))
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
