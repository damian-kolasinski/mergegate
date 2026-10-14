use super::{Grammar, Pack};

/// The grammar that highlights `path`, a repo-relative path: the first pack
/// naming its file name, else the first naming its extension.
pub fn grammar_for<'a>(path: &str, packs: &'a [Pack]) -> Option<(&'a Pack, &'a Grammar)> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let all = || {
        packs
            .iter()
            .flat_map(|p| p.grammars.iter().map(move |g| (p, g)))
    };
    if let Some(found) = all().find(|(_, g)| g.filenames.iter().any(|f| f == name)) {
        return Some(found);
    }
    // A leading dot starts a hidden file's name, not an extension.
    let (stem, extension) = name.rsplit_once('.')?;
    if stem.is_empty() {
        return None;
    }
    all().find(|(_, g)| {
        g.extensions
            .iter()
            .any(|e| e.eq_ignore_ascii_case(extension))
    })
}

#[cfg(test)]
#[path = "grammar_tests.rs"]
mod tests;
