use std::collections::BTreeMap;

use super::{Pack, grammar_for};

/// A pack a repo's files would use, with how many of them it highlights.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    pub pack: String,
    pub files: usize,
}

/// The packs of `known` that highlight any of `paths` (repo-relative), most
/// files first, then by name.
pub fn detect<'a>(paths: impl IntoIterator<Item = &'a str>, known: &[Pack]) -> Vec<Detected> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for path in paths {
        if let Some((pack, _)) = grammar_for(path, known) {
            *counts.entry(&pack.name).or_default() += 1;
        }
    }
    let mut detected: Vec<Detected> = counts
        .into_iter()
        .map(|(pack, files)| Detected {
            pack: pack.to_owned(),
            files,
        })
        .collect();
    detected.sort_by(|a, b| b.files.cmp(&a.files).then_with(|| a.pack.cmp(&b.pack)));
    detected
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod tests;
