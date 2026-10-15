//! Installing the packs released with this mg by name, and finding the
//! installed ones it didn't release.

use std::path::Path;

use super::{Highlighter, Installed, LangError, Pack, PackSource, Release};

/// The packs of `known` that `releases` holds at the same version: what mg
/// can download, with the files each highlights.
pub fn available(known: &[Pack], releases: &[Release]) -> Vec<Pack> {
    known
        .iter()
        .filter(|p| {
            releases
                .iter()
                .any(|r| r.name == p.name && r.version == p.version)
        })
        .cloned()
        .collect()
}

/// The releases of `names`, failing on the first one this mg didn't release.
pub fn resolve(releases: &[Release], names: &[String]) -> Result<Vec<Release>, LangError> {
    if releases.is_empty() {
        return Err(LangError::NoReleases);
    }
    let mut found: Vec<Release> = Vec::new();
    for name in names {
        let release = releases
            .iter()
            .find(|r| &r.name == name)
            .ok_or_else(|| LangError::NotReleased(name.clone()))?;
        if !found.contains(release) {
            found.push(release.clone());
        }
    }
    Ok(found)
}

/// Installed packs this mg released another archive of, by name: built for
/// another mg, so their grammars may not load in this one.
pub fn outdated(installed: &Installed, releases: &[Release]) -> Vec<String> {
    installed
        .packs
        .iter()
        .filter(|p| {
            releases
                .iter()
                .any(|r| r.name == p.name && installed.digests.get(&p.name) != Some(&r.sha256))
        })
        .map(|p| p.name.clone())
        .collect()
}

/// Downloads `names` into the empty directory `into` and installs them
/// together: nothing is installed unless every one downloads and loads.
pub fn install(
    source: &impl PackSource,
    highlighter: &impl Highlighter,
    names: &[String],
    into: &Path,
) -> Result<Vec<Pack>, LangError> {
    let releases = resolve(source.releases(), names)?;
    if releases.is_empty() {
        return Ok(Vec::new());
    }
    for release in &releases {
        source.download(release, into)?;
    }
    highlighter.install(into)
}

#[cfg(test)]
#[path = "released_tests.rs"]
mod tests;
