//! `mg skill install`: writes the agent skill embedded in this `mg`. Where
//! an agent loads skills from differs per agent, so it moves the folder
//! there itself.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use serde::Serialize;

use crate::output::{CliError, Outcome};

#[derive(Subcommand)]
pub enum SkillCommand {
    /// Write the skill folder and print where it is
    Install {
        /// Write it to PATH/mergegate [default: $MERGEGATE_HOME/skill]
        #[arg(long, value_name = "PATH")]
        dir: Option<PathBuf>,
    },
}

#[derive(Serialize)]
struct Installed {
    path: String,
    /// Relative to `path`.
    files: Vec<&'static str>,
    next: &'static str,
}

const NEXT: &str = "Copy or link the folder to where you load skills from. Then ask your \
operator what to do with an item's branch after it merges: mergegate squash-merges, so git \
still sees the branch as unmerged. The default is `git fetch --prune mergegate`, `git switch --detach \
mergegate/<target>`, then `git branch -D <branch>`, which leaves the clone on the updated target. \
Record what you agree where you keep your standing instructions.";

pub fn run(command: SkillCommand) -> Result<Outcome, CliError> {
    let SkillCommand::Install { dir } = command;
    let parent = match dir {
        Some(dir) => std::path::absolute(&dir).map_err(failed(&dir))?,
        None => crate::home()?.root().join("skill"),
    };
    let path = parent.join(skill::NAME);
    install(&parent, &path)?;
    let installed = Installed {
        path: path.display().to_string(),
        files: skill::FILES.iter().map(|(file, _)| *file).collect(),
        next: NEXT,
    };
    let mut text = format!("installed the {} skill in {}", skill::NAME, installed.path);
    for file in &installed.files {
        text.push_str(&format!("\n  {file}"));
    }
    text.push_str(&format!("\n\n{NEXT}"));
    Ok(Outcome::done(&installed, text))
}

/// Writes the folder next to `path` and swaps it in, so a reader never sees
/// half of it and files a newer skill dropped go.
fn install(parent: &Path, path: &Path) -> Result<(), CliError> {
    if path.exists() && !is_skill(path) {
        return Err(CliError::new(
            "not_a_skill",
            format!(
                "{} exists and isn't the {} skill; not replacing it",
                path.display(),
                skill::NAME
            ),
        )
        .hint("mg skill install --dir PATH"));
    }
    fs::create_dir_all(parent).map_err(failed(parent))?;
    let staging = tempfile::Builder::new()
        .prefix(".mergegate-skill-")
        .tempdir_in(parent)
        .map_err(failed(parent))?;
    for (file, contents) in skill::FILES {
        let target = staging.path().join(file);
        if let Some(dir) = target.parent() {
            fs::create_dir_all(dir).map_err(failed(dir))?;
        }
        fs::write(&target, contents).map_err(failed(&target))?;
    }
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path).map_err(failed(path))?,
        Ok(_) => fs::remove_file(path).map_err(failed(path))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(failed(path)(e)),
    }
    fs::rename(staging.keep(), path).map_err(failed(path))
}

/// A folder (or link to one) an earlier install wrote.
fn is_skill(path: &Path) -> bool {
    fs::read_to_string(path.join("SKILL.md"))
        .is_ok_and(|skill| skill.starts_with(&format!("---\nname: {}\n", skill::NAME)))
}

fn failed(path: &Path) -> impl FnOnce(io::Error) -> CliError + '_ {
    move |e| {
        CliError::new(
            "skill_install_failed",
            format!("writing {}: {e}", path.display()),
        )
    }
}
