use serde::{Deserialize, Serialize};

/// `GET /api/hub/settings`, and the answer to [`SetSetting`].
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// How long a merged or abandoned item keeps its attachments, as in `2d`.
    pub attachment_ttl: String,
    /// `local`, or `tailnet` to listen on the tailnet as well.
    pub listen: String,
    /// What the hub's listener on the tailnet is doing; none while `listen`
    /// is `local`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub tailnet: Option<Tailnet>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tailnet {
    /// The URL other machines connect to, once the hub listens there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub url: Option<String>,
    /// Why it doesn't listen yet; the hub retries until it does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub problem: Option<String>,
}

/// `PUT /api/hub/settings/{key}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetSetting {
    pub value: String,
}
