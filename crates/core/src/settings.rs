//! Hub-wide settings, changed with `mg hub set` (design §6.0). Each has a
//! default, so a hub that never set one reads it.

mod ttl;

pub use ttl::{Ttl, TtlError};

use crate::store::StoreError;

/// The keys `mg hub set` accepts.
pub const KEYS: &[&str] = &[ATTACHMENT_TTL, LISTEN];

const ATTACHMENT_TTL: &str = "attachment-ttl";
const LISTEN: &str = "listen";

pub trait SettingsStore {
    /// The value last set for `key`, as it was given.
    fn setting(&self, key: &str) -> Result<Option<String>, StoreError>;

    fn set_setting(&mut self, key: &str, value: &str) -> Result<(), StoreError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// How long a closed item keeps its attachments.
    pub attachment_ttl: Ttl,
    pub listen: Listen,
}

/// Where the hub listens besides 127.0.0.1 (design §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Listen {
    #[default]
    Local,
    /// Also on the machine's Tailscale address.
    Tailnet,
}

impl Listen {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "local" => Some(Self::Local),
            "tailnet" => Some(Self::Tailnet),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Tailnet => "tailnet",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("there is no hub setting {key:?}; settings: {keys}", keys = KEYS.join(", "))]
    Unknown { key: String },
    #[error("{key} can't be {value:?}; expected {expected}")]
    InvalidValue {
        key: &'static str,
        value: String,
        expected: &'static str,
    },
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Every setting, the defaults standing in for those never set.
pub fn settings<S: SettingsStore>(store: &S) -> Result<Settings, SettingsError> {
    let attachment_ttl = match store.setting(ATTACHMENT_TTL)? {
        Some(value) => parse_ttl(&value)?,
        None => Ttl::DEFAULT_ATTACHMENTS,
    };
    let listen = match store.setting(LISTEN)? {
        Some(value) => parse_listen(&value)?,
        None => Listen::default(),
    };
    Ok(Settings {
        attachment_ttl,
        listen,
    })
}

/// Checks `value` and stores it, then reads every setting back.
pub fn set<S: SettingsStore>(
    store: &mut S,
    key: &str,
    value: &str,
) -> Result<Settings, SettingsError> {
    let canonical = match key {
        ATTACHMENT_TTL => parse_ttl(value)?.to_string(),
        LISTEN => parse_listen(value)?.as_str().to_owned(),
        _ => return Err(SettingsError::Unknown { key: key.into() }),
    };
    store.set_setting(key, &canonical)?;
    settings(store)
}

fn parse_ttl(value: &str) -> Result<Ttl, SettingsError> {
    Ttl::parse(value).map_err(|e| SettingsError::InvalidValue {
        key: ATTACHMENT_TTL,
        value: value.into(),
        expected: match e {
            TtlError::Malformed => "a duration such as 2d, 12h, 30m or 0s",
            // Ttl::MAX as it prints, which a test holds this to.
            TtlError::TooLong => "at most 3650d",
        },
    })
}

fn parse_listen(value: &str) -> Result<Listen, SettingsError> {
    Listen::parse(value).ok_or_else(|| SettingsError::InvalidValue {
        key: LISTEN,
        value: value.into(),
        expected: "local or tailnet",
    })
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
