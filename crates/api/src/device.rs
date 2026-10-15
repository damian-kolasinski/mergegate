use serde::{Deserialize, Serialize};

/// `GET /api/whoami`: who the hub takes the caller for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhoAmI {
    pub via: Via,
    /// The caller's tailnet device; none over loopback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<Caller>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    Loopback,
    Tailnet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caller {
    /// Its MagicDNS short name.
    pub name: String,
    /// The Tailscale login it belongs to, or its tags.
    pub owner: String,
    pub trust: Trust,
}

/// Why a device may reach the hub.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trust {
    /// It belongs to the Tailscale user the hub's machine belongs to.
    Owner,
    /// The operator allowed it.
    Allowed,
}

/// `GET /api/hub/devices`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Devices {
    /// Every device on the tailnet, and every one the hub kept that left it,
    /// by name.
    pub devices: Vec<Device>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub name: String,
    pub owner: String,
    pub standing: Standing,
    pub on_tailnet: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Standing {
    Owner,
    Allowed,
    Denied,
    /// Refused, and the operator hasn't decided yet.
    Asked,
    NotAllowed,
}

/// `PUT /api/hub/devices/{name}`, where `name` is the device's short or full
/// MagicDNS name or one of its addresses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetAccess {
    pub access: Access,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    Allow,
    Deny,
}

/// The answer to [`SetAccess`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decided {
    pub name: String,
    pub owner: String,
    pub access: Access,
}
