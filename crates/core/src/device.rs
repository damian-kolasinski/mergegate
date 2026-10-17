//! Devices on the operator's tailnet, and which of them may reach the hub.
//! Tailscale says who a caller is; the hub's owner and the
//! operator's `mg hub allow` and `deny` say whether it may.

use std::net::IpAddr;

use crate::store::StoreError;

/// Tailscale's stable node ID, which survives renaming the device.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    User {
        id: u64,
        login: String,
    },
    /// A tagged device belongs to no user.
    Tags(Vec<String>),
}

impl Owner {
    /// The login, or the tags, as the operator reads them.
    pub fn describe(&self) -> String {
        match self {
            Owner::User { login, .. } => login.clone(),
            Owner::Tags(tags) => tags.join(","),
        }
    }
}

/// A device as Tailscale describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub id: DeviceId,
    /// Its MagicDNS short name, as in `worker-2`.
    pub name: String,
    /// Its full MagicDNS name, without the trailing dot.
    pub dns_name: String,
    pub owner: Owner,
    pub ips: Vec<IpAddr>,
}

/// The tailnet as the hub's machine sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// The hub's own machine.
    pub me: Device,
    pub peers: Vec<Device>,
}

/// The port to the operator's Tailscale.
pub trait Tailnet {
    /// Asks now; nothing cached.
    fn status(&self) -> Result<Status, TailnetError>;

    /// The device that owns `ip` on the tailnet.
    fn whois(&self, ip: IpAddr) -> Result<Device, TailnetError>;
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum TailnetError {
    /// Not installed, not running or logged out.
    #[error("{0}")]
    Unavailable(String),
    #[error("no device on the tailnet has the address {0}")]
    Unknown(IpAddr),
}

/// What the operator decided about a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Allow,
    Deny,
}

/// What the hub keeps about a device: the operator's decision, and when it
/// was last refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub id: DeviceId,
    /// As Tailscale last named it.
    pub name: String,
    pub owner: String,
    pub access: Option<Access>,
    /// Unix seconds.
    pub asked_at: Option<u64>,
}

pub trait DeviceStore {
    fn device(&self, id: &DeviceId) -> Result<Option<Record>, StoreError>;

    /// Every device the hub keeps, by name.
    fn devices(&self) -> Result<Vec<Record>, StoreError>;

    /// Inserts `record`, or replaces the one with its ID.
    fn put_device(&mut self, record: &Record) -> Result<(), StoreError>;
}

/// Why a device may reach the hub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// It belongs to the Tailscale user the hub's machine belongs to.
    Owner,
    Allowed,
}

/// Why a device may not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Denied,
    NotAllowed,
}

/// Where a device stands, as `mg hub devices` lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Owner,
    Allowed,
    Denied,
    /// Refused, and the operator hasn't decided yet.
    Asked,
    NotAllowed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub name: String,
    pub owner: String,
    pub standing: Standing,
    /// Whether it is on the tailnet now; a kept device may have left.
    pub on_tailnet: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("no device named {name:?} on the tailnet")]
    Unknown { name: String },
    #[error("{name:?} names more than one device: {}", full.join(", "))]
    Ambiguous { name: String, full: Vec<String> },
    #[error(transparent)]
    Tailnet(#[from] TailnetError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The rule: a decision stands, and without one only the hub owner's own
/// devices get in. A deny holds even against the owner's devices.
pub fn trust(me: &Device, caller: &Device, access: Option<Access>) -> Result<Trust, Refusal> {
    match access {
        Some(Access::Deny) => Err(Refusal::Denied),
        Some(Access::Allow) => Ok(Trust::Allowed),
        None if same_user(&me.owner, &caller.owner) => Ok(Trust::Owner),
        None => Err(Refusal::NotAllowed),
    }
}

fn same_user(a: &Owner, b: &Owner) -> bool {
    matches!((a, b), (Owner::User { id: a, .. }, Owner::User { id: b, .. }) if a == b)
}

/// Decides on `caller` for the hub on `me`, and remembers a refusal so the
/// operator sees who asked.
pub fn admit(
    store: &mut impl DeviceStore,
    me: &Device,
    caller: &Device,
    now: u64,
) -> Result<Result<Trust, Refusal>, StoreError> {
    let record = store.device(&caller.id)?;
    let access = record.as_ref().and_then(|r| r.access);
    let decision = trust(me, caller, access);
    if decision.is_err() {
        store.put_device(&Record {
            id: caller.id.clone(),
            name: caller.name.clone(),
            owner: caller.owner.describe(),
            access,
            asked_at: Some(now),
        })?;
    }
    Ok(decision)
}

/// Records the operator's decision on the device `name` names: its short or
/// full MagicDNS name, or one of its addresses.
pub fn decide(
    store: &mut impl DeviceStore,
    tailnet: &impl Tailnet,
    name: &str,
    access: Access,
) -> Result<Record, DeviceError> {
    let status = tailnet.status()?;
    let device = find(&status, name)?;
    let asked_at = store.device(&device.id)?.and_then(|r| r.asked_at);
    let record = Record {
        id: device.id.clone(),
        name: device.name.clone(),
        owner: device.owner.describe(),
        access: Some(access),
        asked_at,
    };
    store.put_device(&record)?;
    Ok(record)
}

fn find<'a>(status: &'a Status, name: &str) -> Result<&'a Device, DeviceError> {
    let name = name.trim_end_matches('.');
    let devices = || std::iter::once(&status.me).chain(&status.peers);
    let exact = devices().find(|d| {
        d.dns_name.eq_ignore_ascii_case(name) || d.ips.iter().any(|ip| ip.to_string() == name)
    });
    if let Some(device) = exact {
        return Ok(device);
    }
    // Devices shared in from other tailnets can share a short name, and a
    // decision on the wrong one would let it in.
    let named: Vec<_> = devices()
        .filter(|d| d.name.eq_ignore_ascii_case(name))
        .collect();
    match named[..] {
        [device] => Ok(device),
        [] => Err(DeviceError::Unknown { name: name.into() }),
        _ => Err(DeviceError::Ambiguous {
            name: name.into(),
            full: named.iter().map(|d| d.dns_name.clone()).collect(),
        }),
    }
}

/// Every device on the tailnet, and every device the hub kept that has left
/// it, by name.
pub fn list(status: &Status, records: &[Record]) -> Vec<Listed> {
    let on_tailnet = std::iter::once(&status.me)
        .chain(&status.peers)
        .map(|device| {
            let record = records.iter().find(|r| r.id == device.id);
            let standing = match trust(&status.me, device, record.and_then(|r| r.access)) {
                Ok(Trust::Owner) => Standing::Owner,
                Ok(Trust::Allowed) => Standing::Allowed,
                Err(Refusal::Denied) => Standing::Denied,
                Err(Refusal::NotAllowed) if record.is_some_and(|r| r.asked_at.is_some()) => {
                    Standing::Asked
                }
                Err(Refusal::NotAllowed) => Standing::NotAllowed,
            };
            Listed {
                name: device.name.clone(),
                owner: device.owner.describe(),
                standing,
                on_tailnet: true,
            }
        });
    let gone = records
        .iter()
        .filter(|r| r.id != status.me.id && !status.peers.iter().any(|d| d.id == r.id))
        .map(|r| Listed {
            name: r.name.clone(),
            owner: r.owner.clone(),
            standing: match r.access {
                Some(Access::Allow) => Standing::Allowed,
                Some(Access::Deny) => Standing::Denied,
                None => Standing::Asked,
            },
            on_tailnet: false,
        });
    let mut listed: Vec<_> = on_tailnet.chain(gone).collect();
    listed.sort_by(|a, b| a.name.cmp(&b.name));
    listed
}

#[cfg(test)]
#[path = "device_tests.rs"]
mod tests;
