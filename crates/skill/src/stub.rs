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

/// How every whole skill an `mg` has shipped starts its description, which
/// a skill the operator wrote under the same name wouldn't. A new
/// description keeps this line or extends the check.
const SHIPPED: &str = "description: Submit work for review through the operator's mergegate hub";

/// What marks a stub, of any version.
const MARKER: &str = "\n  mergegate-stub: \"";

/// Whether `skill_md` is a `SKILL.md` some `mg` wrote: this stub, an older
/// one, or the whole skill `mg skill install` writes.
pub fn is_ours(skill_md: &str) -> bool {
    named(skill_md).is_some_and(|rest| rest.starts_with(SHIPPED) || skill_md.contains(MARKER))
}

/// Whether `skill_md` is a stub, of any version.
pub(crate) fn is_stub(skill_md: &str) -> bool {
    named(skill_md).is_some() && skill_md.contains(MARKER)
}

/// What follows the skill's name in `skill_md`'s frontmatter, if it names
/// this skill.
fn named(skill_md: &str) -> Option<&str> {
    skill_md.strip_prefix(&format!("---\nname: {NAME}\n"))
}

/// Where in the skill folder an older guide had Codex agents point their
/// wake, `python3 <folder>/scripts/codex_wake.py`. Every folder setup
/// writes or retires keeps it there, so those wakes still run.
pub(crate) const WAKE_SCRIPT: &str = "scripts/codex_wake.py";

/// The script at [`WAKE_SCRIPT`], which hands over to `mg wakes codex`.
pub(crate) fn wake_script() -> &'static str {
    guide::FILES
        .iter()
        .find(|(path, _)| *path == WAKE_SCRIPT)
        .map(|(_, text)| *text)
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "stub_tests.rs"]
mod tests;
