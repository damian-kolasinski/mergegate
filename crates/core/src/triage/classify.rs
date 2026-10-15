//! Placing one changed file in a tier: the guard, then the rules in order.

use super::rules::Compiled;
use super::{Guard, Kind, Rules, Tier, Triage, TriagedBy};
use crate::diff::{Content, DiffStore, LineKind};
use crate::item::{ChangedFile, FileMode, FileStat};
use crate::store::StoreError;

pub const GITATTRIBUTES: &str = ".gitattributes";

/// `content` reads the file's diff, only when a rule needs it; none when the
/// diff isn't cached.
///
/// Where a matcher can't tell (a rename's two paths, lines it can't count or
/// read), it answers whatever keeps the file in `review`: a `trusted` rule
/// doesn't match and a `review` rule does. So uncertainty never hides a file.
/// No `trusted` rule matches a mode or type change, a symlink or a
/// submodule, which no matcher reads.
pub fn classify<E>(
    rules: &Rules,
    file: &FileStat,
    content: impl FnOnce() -> Result<Option<Content>, E>,
) -> Result<Triage, E> {
    let paths = || std::iter::once(&file.file.path).chain(&file.file.old_path);
    if paths().any(|p| p.rsplit('/').next() == Some(GITATTRIBUTES)) {
        return Ok(Triage {
            tier: Tier::Review,
            by: Some(TriagedBy::Guard(Guard::Gitattributes)),
        });
    }
    let mut content = Lazy::Unread(Some(content));
    for compiled in rules.compiled() {
        if matches(compiled, file, &mut content)? {
            let rule = &compiled.rule;
            return Ok(Triage {
                tier: rule.tier,
                by: Some(TriagedBy::Rule {
                    id: rule.id,
                    why: format!("{} — {}", rule.summary(), rule.reason),
                }),
            });
        }
    }
    Ok(Triage::default())
}

/// Classifies every file in place, reading cached diffs from `store`.
pub fn triage_files(
    store: &impl DiffStore,
    rules: &Rules,
    files: &mut [FileStat],
) -> Result<(), StoreError> {
    for file in files {
        let (old, new) = (file.file.old_blob.as_deref(), file.file.new_blob.as_deref());
        file.triage = classify(rules, file, || {
            Ok::<_, StoreError>(store.diff(old, new)?.map(|diff| diff.content))
        })?;
    }
    Ok(())
}

enum Lazy<F> {
    Unread(Option<F>),
    Read(Option<Content>),
}

impl<E, F: FnOnce() -> Result<Option<Content>, E>> Lazy<F> {
    fn get(&mut self) -> Result<Option<&Content>, E> {
        if let Self::Unread(read) = self {
            let read = read.take().expect("read once");
            *self = Self::Read(read()?);
        }
        match self {
            Self::Read(content) => Ok(content.as_ref()),
            Self::Unread(_) => unreachable!("read above"),
        }
    }
}

fn matches<E, F: FnOnce() -> Result<Option<Content>, E>>(
    compiled: &Compiled,
    file: &FileStat,
    content: &mut Lazy<F>,
) -> Result<bool, E> {
    let rule = &compiled.rule;
    let unsure = rule.tier == Tier::Review;
    if rule.tier == Tier::Trusted && unreadable_mode(&file.file) {
        return Ok(false);
    }
    if let Some(glob) = &compiled.path {
        let new = glob.is_match(&file.file.path);
        let matched = match &file.file.old_path {
            None => new,
            // A rename can carry code out of a reviewed directory, so hiding
            // it takes both of its paths.
            Some(old) if rule.tier == Tier::Trusted => new && glob.is_match(old),
            Some(old) => new || glob.is_match(old),
        };
        if !matched {
            return Ok(false);
        }
    }
    if let Some(kind) = rule.kind
        && !is(kind, file)
    {
        return Ok(false);
    }
    if let Some(max) = rule.max_lines {
        let fits = match (file.additions, file.deletions) {
            (Some(added), Some(removed)) => added + removed <= max,
            _ => unsure,
        };
        if !fits {
            return Ok(false);
        }
    }
    if let Some(regex) = &compiled.diff {
        let found = match content.get()? {
            Some(Content::Text(hunks)) => {
                let mut changed = hunks
                    .iter()
                    .flat_map(|h| &h.lines)
                    .filter(|l| l.kind != LineKind::Context)
                    .peekable();
                // Reviewing a file takes one matching line; hiding it takes
                // every changed line, so one import can't hide a rewrite.
                match rule.tier {
                    Tier::Review => changed.any(|l| regex.is_match(&l.text)),
                    Tier::Trusted => {
                        changed.peek().is_some() && changed.all(|l| regex.is_match(&l.text))
                    }
                }
            }
            _ => unsure,
        };
        if !found {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Whether `file`'s modes say what no matcher reads: a changed exec bit or
/// type shows in no line, a symlink's one line can point anywhere, and a
/// submodule's diff is another repository's. A file recorded before modes
/// were kept has none, and is matched by its lines as it was then.
fn unreadable_mode(file: &ChangedFile) -> bool {
    let sides = [file.old_mode, file.new_mode];
    let changed = matches!(sides, [Some(old), Some(new)] if old != new);
    changed
        || sides
            .iter()
            .flatten()
            .any(|m| matches!(m, FileMode::Symlink | FileMode::Submodule))
}

fn is(kind: Kind, file: &FileStat) -> bool {
    let f = &file.file;
    match kind {
        Kind::Added => f.old_blob.is_none(),
        Kind::Deleted => f.new_blob.is_none(),
        Kind::Renamed => f.old_path.is_some(),
        Kind::Modified => f.old_path.is_none() && f.old_blob.is_some() && f.new_blob.is_some(),
        Kind::Generated => file.generated,
    }
}

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;
