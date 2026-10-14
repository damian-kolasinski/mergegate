//! The packs `langs/packs.json` builds: what mg can name when a file's
//! language isn't installed.

use mergegate_core::highlight::Pack;
use serde::Deserialize;

use crate::manifest::GrammarManifest;

#[derive(Deserialize)]
struct Catalogue {
    packs: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    name: String,
    version: String,
    grammars: Vec<GrammarManifest>,
}

/// By name.
pub fn known() -> Vec<Pack> {
    let catalogue: Catalogue = serde_json::from_str(include_str!("../../../langs/packs.json"))
        .expect("langs/packs.json is checked by this crate's tests");
    let mut packs: Vec<Pack> = catalogue
        .packs
        .into_iter()
        .map(|e| Pack {
            name: e.name,
            version: e.version,
            grammars: e.grammars.into_iter().map(Into::into).collect(),
        })
        .collect();
    packs.sort_by(|a, b| a.name.cmp(&b.name));
    packs
}

#[cfg(test)]
#[path = "catalogue_tests.rs"]
mod tests;
