use serde::{Deserialize, Serialize};

/// `PUT /api/items/{id}/viewed`: marks or unmarks one of a revision's files.
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkViewed {
    pub revision: u32,
    pub path: String,
    pub viewed: bool,
}
