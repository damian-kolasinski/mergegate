use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Draft {
    pub format_version: u32,
    pub id: String,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub number: u64,
    pub url: String,
}
