//! Hub-wide settings, changed with `mg hub set` (design §6.0). Each has a
//! default, so a hub that never set one reads it.

mod ttl;

pub use ttl::Ttl;

use crate::store::StoreError;

/// The keys `mg hub set` accepts.
pub const KEYS: &[&str] = &[ATTACHMENT_TTL];

const ATTACHMENT_TTL: &str = "attachment-ttl";

pub trait SettingsStore {
    /// The value last set for `key`, as it was given.
    fn setting(&self, key: &str) -> Result<Option<String>, StoreError>;

    fn set_setting(&mut self, key: &str, value: &str) -> Result<(), StoreError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// How long a closed item keeps its attachments.
    pub attachment_ttl: Ttl,
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
    Ok(Settings { attachment_ttl })
}

/// Checks `value` and stores it, then reads every setting back.
pub fn set<S: SettingsStore>(
    store: &mut S,
    key: &str,
    value: &str,
) -> Result<Settings, SettingsError> {
    let canonical = match key {
        ATTACHMENT_TTL => parse_ttl(value)?.to_string(),
        _ => return Err(SettingsError::Unknown { key: key.into() }),
    };
    store.set_setting(key, &canonical)?;
    settings(store)
}

fn parse_ttl(value: &str) -> Result<Ttl, SettingsError> {
    Ttl::parse(value).ok_or_else(|| SettingsError::InvalidValue {
        key: ATTACHMENT_TTL,
        value: value.into(),
        expected: "a duration such as 2d, 12h, 30m or 0s",
    })
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
