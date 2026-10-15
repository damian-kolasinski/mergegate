//! `languages.json`, as `langs/build.mjs` writes it.

use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct Index {
    pub packs: Vec<Entry>,
}

#[derive(Deserialize)]
pub(crate) struct Entry {
    pub name: String,
    pub version: String,
    /// The archive's name among the release's assets.
    pub file: String,
    pub sha256: String,
}

impl Index {
    pub fn parse(text: &str) -> Result<Index, String> {
        let index: Index =
            serde_json::from_str(text).map_err(|e| format!("bad language pack index: {e}"))?;
        for p in &index.packs {
            // Joined to a URL and to the download directory.
            let plain = p.file.ends_with(".tar.gz")
                && !p.file.starts_with('.')
                && !p.file.contains(['/', '\\']);
            if !plain {
                return Err(format!(
                    "bad language pack index: {} is not an archive name",
                    p.file
                ));
            }
            let hex = p.sha256.len() == 64
                && p.sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
            if !hex {
                return Err(format!(
                    "bad language pack index: {} has no SHA-256",
                    p.name
                ));
            }
        }
        Ok(index)
    }
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;
