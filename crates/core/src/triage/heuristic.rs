//! Structural heuristics: a guess at how much a file needs the operator from
//! its path and diff alone, so files rank sensibly with no guide and no judge.
//! They only rank; a file's tier is the rules' alone.

use std::sync::LazyLock;

use regex::Regex;

use crate::diff::{Content, LineKind};

/// Why the heuristics moved a file, the first that applies in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hint {
    Lockfile,
    Generated,
    Tests,
    Docs,
    /// Removes lines and adds none.
    OnlyDeletes,
    Migration,
    /// Adds or removes a declaration line: a function, type or export.
    Signature,
    Config,
}

impl Hint {
    /// What the UI shows beside a file nothing else explains.
    pub fn describe(self) -> &'static str {
        match self {
            Self::Lockfile => "ranked low: lockfile",
            Self::Generated => "ranked low: generated",
            Self::Tests => "ranked low: tests",
            Self::Docs => "ranked low: docs",
            Self::OnlyDeletes => "ranked low: only removes lines",
            Self::Migration => "ranked high: migration",
            Self::Signature => "ranked high: changes a declaration",
            Self::Config => "ranked high: config",
        }
    }

    /// Higher ranks first; a file with no hint weighs 0.
    pub fn weight(self) -> i32 {
        match self {
            Self::Lockfile => -40,
            Self::Generated => -35,
            Self::Docs => -25,
            Self::Tests => -20,
            Self::OnlyDeletes => -15,
            Self::Config => 10,
            Self::Signature => 20,
            Self::Migration => 30,
        }
    }
}

const LOCKFILES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lockb",
    "Gemfile.lock",
    "poetry.lock",
    "uv.lock",
    "composer.lock",
    "go.sum",
    "Package.resolved",
    "Podfile.lock",
    "flake.lock",
    "mix.lock",
];

const CONFIGS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "tsconfig.json",
    "Dockerfile",
    "Makefile",
    "build.gradle",
    "build.gradle.kts",
    "Package.swift",
    "pyproject.toml",
    "go.mod",
    "CMakeLists.txt",
    "requirements.txt",
];

/// A line that declares something callers see. Changing one is a contract
/// change, which is worth a closer look than a body edit.
static DECLARATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\s*(?:export\s|pub(?:\([^)]*\))?\s|public\s|protected\s|open\s|(?:async\s+)?(?:fn|func|def|function|class|struct|enum|trait|interface|protocol|type)\s)",
    )
    .expect("valid regex")
});

/// `content` is none when the diff isn't read, as once an item is abandoned.
pub fn hint(
    path: &str,
    generated: bool,
    additions: Option<u64>,
    deletions: Option<u64>,
    content: Option<&Content>,
) -> Option<Hint> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let dirs: Vec<&str> = path.split('/').rev().skip(1).collect();
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    let ext = ext.as_deref();
    if LOCKFILES.contains(&name) || ext == Some("lock") {
        return Some(Hint::Lockfile);
    }
    if generated {
        return Some(Hint::Generated);
    }
    if is_test(name, &dirs) {
        return Some(Hint::Tests);
    }
    // A `.txt` config is config wherever it sits.
    let docs = matches!(ext, Some("md" | "mdx" | "rst" | "adoc" | "txt"))
        || dirs.iter().any(|d| matches!(*d, "docs" | "doc"));
    if docs && !CONFIGS.contains(&name) {
        return Some(Hint::Docs);
    }
    if ext == Some("sql") || dirs.iter().any(|d| matches!(*d, "migrations" | "migrate")) {
        return Some(Hint::Migration);
    }
    if additions == Some(0) && deletions.is_some_and(|d| d > 0) {
        return Some(Hint::OnlyDeletes);
    }
    if let Some(Content::Text(hunks)) = content {
        let declares = hunks
            .iter()
            .flat_map(|h| &h.lines)
            .any(|l| l.kind != LineKind::Context && DECLARATION.is_match(&l.text));
        if declares {
            return Some(Hint::Signature);
        }
    }
    let config = CONFIGS.contains(&name)
        || name.starts_with(".env")
        || matches!(ext, Some("toml" | "yaml" | "yml" | "ini" | "conf"))
        || dirs.windows(2).any(|w| w == ["workflows", ".github"]);
    config.then_some(Hint::Config)
}

/// `dirs` are the path's directories, innermost first.
fn is_test(name: &str, dirs: &[&str]) -> bool {
    if dirs
        .iter()
        .any(|d| matches!(*d, "test" | "tests" | "__tests__" | "spec" | "e2e"))
    {
        return true;
    }
    let stem = name.split('.').next().unwrap_or(name);
    name.contains(".test.")
        || name.contains(".spec.")
        || stem.ends_with("_test")
        || stem.ends_with("_tests")
        || stem.ends_with("Test")
        || stem.ends_with("Tests")
        || stem.starts_with("test_")
}

#[cfg(test)]
#[path = "heuristic_tests.rs"]
mod tests;
