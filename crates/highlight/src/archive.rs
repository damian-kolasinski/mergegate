//! Unpacking a pack's `.tar.gz`. Archives come from anywhere the operator
//! points `mg lang add`, so only plain files with plain names are written,
//! all at the top of the target directory.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path};

use flate2::read::GzDecoder;
use tar::{Archive, EntryType};

/// Larger than any grammar's wasm; caps what a hostile archive can write.
const MAX_FILE: u64 = 64 << 20;

/// Unpacks `archive` into the empty directory `into`.
pub(crate) fn unpack(archive: &Path, into: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| e.to_string())?;
    let mut tar = Archive::new(GzDecoder::new(file));
    for entry in tar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        let mut parts = path.components().filter(|c| *c != Component::CurDir);
        let name = match (parts.next(), parts.next()) {
            (None, _) if entry.header().entry_type() == EntryType::Directory => continue,
            (Some(Component::Normal(name)), None) => name.to_string_lossy().into_owned(),
            _ => return Err(format!("unexpected entry {}", path.display())),
        };
        if entry.header().entry_type() != EntryType::Regular {
            return Err(format!("{name} is not a plain file"));
        }
        let safe = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
        if name.starts_with('.') || !name.chars().all(safe) {
            return Err(format!("unexpected file name {name:?}"));
        }
        let target = into.join(&name);
        if target.exists() {
            return Err(format!("{name} twice"));
        }
        let mut out = File::create(&target).map_err(|e| format!("{name}: {e}"))?;
        let written = io::copy(&mut (&mut entry).take(MAX_FILE + 1), &mut out)
            .map_err(|e| format!("{name}: {e}"))?;
        if written > MAX_FILE {
            return Err(format!("{name} is larger than {} MiB", MAX_FILE >> 20));
        }
    }
    if !into.join("pack.json").is_file() {
        return Err("no pack.json".into());
    }
    Ok(())
}

/// `pack.json` of the unpacked pack in `dir`.
pub(crate) fn manifest(dir: &Path) -> Result<String, String> {
    fs::read_to_string(dir.join("pack.json")).map_err(|e| format!("pack.json: {e}"))
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
