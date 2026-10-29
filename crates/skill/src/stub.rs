//! The `SKILL.md` that `mg skill setup` puts where agents load skills: the
//! skill's name and description, so agents know when to load it, and a
//! pointer to `mg skill show`, which prints the guide of whichever `mg` is
//! installed. Upgrading `mg` updates what agents read; the stub itself
//! changes only when this text does.

use crate::guide::{self, NAME};

/// Bumped with any change to the stub's text, so an agent's folder shows
/// which `mg` wrote it.
const VERSION: &str = "1";

pub fn text() -> String {
    format!(
        "---\n\
         name: {NAME}\n\
         description: {description}\n\
         metadata:\n  mergegate-stub: \"{VERSION}\"\n\
         ---\n\
         <!-- Written by `mg skill setup`, which keeps it current; `mg skill remove` removes it. -->\n\
         \n\
         # mergegate\n\
         \n\
         Never `git push` and never `gh pr create`: `mg submit` puts work up for review on the \
         operator's mergegate hub.\n\
         \n\
         Run `mg skill show` and follow the guide it prints. It comes from the installed `mg`, \
         so it describes that version's commands; `mg skill show NAME` prints each reference \
         the guide names.\n\
         \n\
         If `mg` answers with a usage line offering `-nR` or opens an editor, it is the text \
         editor macOS ships: run `mergegate` wherever the guide says `mg`.\n",
        description = guide::description(),
    )
}

/// Whether `skill_md` is a `SKILL.md` some `mg` wrote: this stub, an older
/// one, or the whole skill `mg skill install` writes.
pub fn is_ours(skill_md: &str) -> bool {
    skill_md.starts_with(&format!("---\nname: {NAME}\n"))
}

/// Whether `skill_md` is a stub, of any version.
pub(crate) fn is_stub(skill_md: &str) -> bool {
    is_ours(skill_md) && skill_md.contains("\n  mergegate-stub: \"")
}

#[cfg(test)]
#[path = "stub_tests.rs"]
mod tests;
