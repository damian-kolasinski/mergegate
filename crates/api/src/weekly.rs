use serde::{Deserialize, Serialize};

/// The hub's local calendar week, Monday through Sunday.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WeeklyClosures {
    /// Local ISO dates (`YYYY-MM-DD`).
    pub monday: String,
    pub today: String,
    pub repositories: Vec<RepositoryClosures>,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryClosures {
    pub project: String,
    /// Merged and abandoned counts, indexed from Monday (0).
    pub days: [u32; 7],
}
