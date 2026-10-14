//! A pack's `pack.json`, as `langs/build.mjs` writes it.

use mergegate_core::highlight::{Grammar, Pack};
use serde::Deserialize;

#[derive(Deserialize)]
struct Manifest {
    name: String,
    version: String,
    grammars: Vec<GrammarManifest>,
}

#[derive(Deserialize)]
pub(crate) struct GrammarManifest {
    name: String,
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    filenames: Vec<String>,
    #[serde(default)]
    aliases: Vec<String>,
}

impl From<GrammarManifest> for Grammar {
    fn from(g: GrammarManifest) -> Grammar {
        Grammar {
            name: g.name,
            extensions: g.extensions,
            filenames: g.filenames,
            aliases: g.aliases,
        }
    }
}

/// The pack `json` describes. Names become directory and file names, so
/// they are held to characters that are safe as both.
pub(crate) fn parse(json: &str) -> Result<Pack, String> {
    let m: Manifest = serde_json::from_str(json).map_err(|e| format!("pack.json: {e}"))?;
    if !name_ok(&m.name, |c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
    }) {
        return Err(format!("bad pack name {:?}", m.name));
    }
    let version_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+');
    if !name_ok(&m.version, version_char) {
        return Err(format!("bad version {:?}", m.version));
    }
    if m.grammars.is_empty() {
        return Err("no grammars".into());
    }
    let mut grammars: Vec<Grammar> = Vec::new();
    for g in m.grammars {
        // The name is also the wasm's `tree_sitter_<name>` symbol.
        if !name_ok(&g.name, |c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'
        }) {
            return Err(format!("bad grammar name {:?}", g.name));
        }
        if grammars.iter().any(|known| known.name == g.name) {
            return Err(format!("grammar {} twice", g.name));
        }
        grammars.push(g.into());
    }
    Ok(Pack {
        name: m.name,
        version: m.version,
        grammars,
    })
}

/// Non-empty, starting with a letter or digit, so never `.` or `..`.
fn name_ok(name: &str, allowed: impl Fn(char) -> bool) -> bool {
    name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_alphanumeric())
        && name.chars().all(allowed)
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
