//! `mg skill install`: writes the whole skill embedded in this `mg`, for an
//! agent `mg skill setup` doesn't know to load from wherever it looks.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::output::{CliError, Outcome};

#[derive(Serialize)]
struct Installed {
    path: String,
    /// Relative to `path`.
    files: Vec<&'static str>,
    next: &'static str,
}

const NEXT: &str = "Copy or link the folder to where your agent loads skills from, and \
run mg skill install again after mg is upgraded. For Claude Code, Codex and OpenCode, \
mg skill setup installs a skill that stays current with mg instead.";

pub(super) fn run(dir: Option<PathBuf>) -> Result<Outcome, CliError> {
    let parent = match dir {
        Some(dir) => std::path::absolute(&dir).map_err(failed(&dir))?,
        None => crate::home()?.root().join("skill"),
    };
    let path = parent.join(skill::NAME);
    if path.exists() && !is_skill(&path) {
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
    skill::export(&path).map_err(failed(&path))?;
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

/// A folder (or link to one) an earlier install wrote.
fn is_skill(path: &Path) -> bool {
    std::fs::read_to_string(path.join("SKILL.md")).is_ok_and(|skill| skill::is_ours(&skill))
}

fn failed(path: &Path) -> impl FnOnce(std::io::Error) -> CliError + '_ {
    move |e| {
        CliError::new(
            "skill_install_failed",
            format!("writing {}: {e}", path.display()),
        )
    }
}
