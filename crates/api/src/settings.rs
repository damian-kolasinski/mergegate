use serde::{Deserialize, Serialize};

/// `GET /api/hub/settings`, and the answer to [`SetSetting`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// How long a merged or abandoned item keeps its attachments, as in `2d`.
    pub attachment_ttl: String,
}

/// `PUT /api/hub/settings/{key}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetSetting {
    pub value: String,
}
