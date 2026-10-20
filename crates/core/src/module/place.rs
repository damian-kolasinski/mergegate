//! Finding a file's module from the folders above it. The nearest folder
//! that looks like a module is one:
//!
//! - a target group, as Tuist and Xcode projects lay modules out: `Chat`
//!   holding a `Chat` target, beside `ChatInterface`, `ChatTests` and the
//!   like;
//! - a package, with a build manifest of its own, as a crate or an npm
//!   package.
//!
//! The repo's root is never one: it would hold every file.

use std::collections::{BTreeMap, BTreeSet};

use super::Entry;

/// Files that make the folder holding them a package.
pub const MANIFESTS: [&str; 13] = [
    "Cargo.toml",
    "Package.swift",
    "Project.swift",
    "package.json",
    "go.mod",
    "build.gradle",
    "build.gradle.kts",
    "pom.xml",
    "pyproject.toml",
    "setup.py",
    "pubspec.yaml",
    "composer.json",
    "mix.exs",
];

/// Manifests named for their package, as `Company.Api.csproj`.
const PROJECT_FILES: [&str; 3] = [".csproj", ".fsproj", ".gemspec"];

fn is_manifest(name: &str) -> bool {
    MANIFESTS.contains(&name) || PROJECT_FILES.iter().any(|ext| name.ends_with(ext))
}

/// Folders that are one thing on disk rather than code laid out in
/// folders, so never a module.
const BUNDLES: [&str; 8] = [
    ".xcassets",
    ".lproj",
    ".bundle",
    ".xcodeproj",
    ".xcworkspace",
    ".framework",
    ".xcframework",
    ".playground",
];

/// Folders directly in a package that hold its code or tests, so a file's
/// path within the package reads the same in either.
const SOURCES: [&str; 3] = ["src", "Sources", "lib"];
const TESTS: [&str; 4] = ["tests", "Tests", "test", "__tests__"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    /// The module's folder.
    pub module: String,
    /// The folder directly in the module the file is in, when it holds one
    /// of the module's targets: `Chat`, `ChatTests` or `src`. The file's
    /// path within the module leaves it out.
    pub target: Option<String>,
}

impl Placement {
    /// The module's name: its folder's.
    pub fn name(&self) -> &str {
        self.module.rsplit('/').next().unwrap_or(&self.module)
    }

    /// What the target holds besides the module's code: `Interface` for
    /// `ChatInterface`, `Tests` for `tests`. None for the code itself.
    pub fn layer(&self) -> Option<&str> {
        let target = self.target.as_deref()?;
        if TESTS.contains(&target) {
            return Some("Tests");
        }
        // A package's sources hold its code even when named like a layer
        // of it, as `Sources` in a package `Source`.
        if SOURCES.contains(&target) {
            return None;
        }
        target
            .strip_prefix(self.name())
            .filter(|rest| !rest.is_empty())
    }

    /// `path` within the module, without the target's folder.
    pub fn inner<'a>(&self, path: &'a str) -> &'a str {
        let within = path
            .strip_prefix(self.module.as_str())
            .and_then(|p| p.strip_prefix('/'))
            .unwrap_or(path);
        match &self.target {
            Some(target) => within
                .strip_prefix(target.as_str())
                .and_then(|p| p.strip_prefix('/'))
                .unwrap_or(within),
            None => within,
        }
    }
}

/// Every folder above any of `paths`, but the root: the ones [`place`]
/// needs listed.
pub fn ancestors<'a>(paths: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    for path in paths {
        let mut end = 0;
        while let Some(slash) = path[end..].find('/') {
            end += slash;
            dirs.insert(path[..end].to_owned());
            end += 1;
        }
    }
    dirs
}

/// `path`'s module, from `listings` of the folders above it. None when no
/// folder above it looks like one.
pub fn place(path: &str, listings: &BTreeMap<String, Vec<Entry>>) -> Option<Placement> {
    let segments: Vec<&str> = path.split('/').collect();
    for depth in (1..segments.len()).rev() {
        let name = segments[depth - 1];
        // A hidden folder or a bundle such as `Assets.xcassets`; a dot
        // elsewhere, as in `socket.io` or `Company.Api`, can be a package.
        if name.starts_with('.') || BUNDLES.iter().any(|b| name.ends_with(b)) {
            continue;
        }
        // A target named for the module it's in.
        if depth >= 2 && segments[depth - 2] == name {
            continue;
        }
        let module = segments[..depth].join("/");
        let Some(entries) = listings.get(&module) else {
            continue;
        };
        let grouped = entries.iter().any(|e| e.dir && e.name == name);
        let package = entries.iter().any(|e| !e.dir && is_manifest(&e.name));
        if !grouped && !package {
            continue;
        }
        // The segment after the module is a folder only when more follow.
        let next = segments.get(depth).filter(|_| depth + 1 < segments.len());
        let target = next
            .filter(|next| is_target(name, next, grouped))
            .map(|next| (*next).to_owned());
        return Some(Placement { module, target });
    }
    None
}

/// `path`'s placement in the nearest of `modules` above it, for a file whose
/// own wasn't found: one an earlier revision changed and the newest dropped.
pub fn adopt<'a>(path: &str, modules: impl IntoIterator<Item = &'a str>) -> Option<Placement> {
    let module = modules
        .into_iter()
        .filter(|m| {
            path.strip_prefix(m)
                .is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(|m| m.len())?;
    let name = module.rsplit('/').next().unwrap_or(module);
    let rest = &path[module.len() + 1..];
    let target = rest
        .split_once('/')
        .map(|(next, _)| next)
        .filter(|next| is_target(name, next, true) || is_target(name, next, false))
        .map(str::to_owned);
    Some(Placement {
        module: module.to_owned(),
        target,
    })
}

/// Whether `folder`, directly in module `name`, holds one of its targets.
/// A target group's are named for it, as `Chat` and `ChatInterface`; a
/// package's are its sources and tests.
fn is_target(name: &str, folder: &str, grouped: bool) -> bool {
    if grouped {
        return folder
            .strip_prefix(name)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(|c: char| c.is_uppercase()));
    }
    SOURCES.contains(&folder) || TESTS.contains(&folder)
}

#[cfg(test)]
#[path = "place_tests.rs"]
mod tests;
