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

/// `POST /api/languages`: installs packs from exactly one source. Answers
/// with [`Packs`], the packs installed, empty when there was nothing to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddPacks {
    /// The pack archive at this absolute path, or every `.tar.gz` in the
    /// directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// These packs, downloaded from mg's release.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<String>,
    /// The packs from mg's release that the files on this project's target
    /// use and that aren't installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detect: Option<String>,
    /// The installed packs mg's release has another archive of, again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub update: bool,
}

/// `GET /api/released-languages`: the packs released with the hub's mg,
/// which `AddPacks::names` installs. Empty for a build without its
/// release's index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Released {
    /// By name.
    pub packs: Vec<ReleasedPack>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasedPack {
    #[serde(flatten)]
    pub pack: Pack,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub installed: bool,
    /// Installed from another archive than the release's.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub outdated: bool,
}
