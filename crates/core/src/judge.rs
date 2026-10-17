//! The judge: an optional model that scores how much each file of a revision
//! needs the operator, and may pull a rule-trusted file back into review.
//! It sees the whole change once per revision, in the
//! background; its verdicts are cached under the setting that produced them
//! and applied when the item is read.

mod pass;
mod request;
mod setting;
mod store;

pub use pass::{Unjudged, accept, request, unjudged};
pub use request::{Change, Request, RequestFile};
pub use setting::{DEFAULT_MODEL, DEFAULT_THRESHOLD, JudgeSetting, Provider, SettingError, set};
pub use store::JudgeStore;

/// A provider that judges a revision's files, called off the store.
pub trait Judge {
    /// A verdict for some or all of `request`'s files. Unknown paths and
    /// out-of-range values are the pass's to drop or clamp.
    fn judge(&self, request: &Request) -> Result<Vec<Verdict>, JudgeError>;
}

/// The judge's word on one file.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub path: String,
    /// 1 to 100.
    pub importance: u8,
    /// One line on why, when the provider gives one.
    pub reason: Option<String>,
    /// For a trusted file, the confidence, 0 to 1, that it needs a human
    /// look anyway.
    pub promote: Option<f64>,
}

/// Why a provider gave no verdicts, as the operator should read it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct JudgeError(pub String);
