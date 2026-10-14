use serde::{Deserialize, Serialize};

/// `GET /api/languages`: the installed language packs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Packs {
    /// By name.
    pub packs: Vec<Pack>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pack {
    pub name: String,
    pub version: String,
    pub grammars: Vec<Grammar>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grammar {
    pub name: String,
    pub extensions: Vec<String>,
    pub filenames: Vec<String>,
}

/// `POST /api/languages`: installs the pack archive at `from`, or every
/// `.tar.gz` in the directory `from`. Answers with [`Packs`], the packs
/// installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddPacks {
    /// Absolute.
    pub from: String,
}
