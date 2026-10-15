use super::{Check, CheckStatus, check};
use crate::git::Git;
use crate::highlight::{Installed, Pack, Release, available, outdated};
use crate::project::{Project, missing};

/// The installed packs, warning about those built for another mg, then a
/// warning for each project whose target uses packs mg released but that
/// aren't installed. A build without releases (a dev build) can't install
/// packs by name, so it only lists them.
pub fn languages(
    git: &impl Git,
    projects: &[Project],
    known: &[Pack],
    installed: &Installed,
    releases: &[Release],
) -> Vec<Check> {
    let names: Vec<String> = installed.packs.iter().map(|p| p.name.clone()).collect();
    let listed = if names.is_empty() {
        "none installed".to_owned()
    } else {
        names.join(", ")
    };
    let outdated = outdated(installed, releases);
    let mut checks = vec![if outdated.is_empty() {
        check("languages", CheckStatus::Ok, listed, None)
    } else {
        check(
            "languages",
            CheckStatus::Warn,
            format!("{} built for another mg", outdated.join(", ")),
            Some("mg lang update".into()),
        )
    }];
    let released = available(known, releases);
    if released.is_empty() {
        return checks;
    }
    for project in projects {
        // A repo that can't be read fails its project check already.
        let Ok(missing) = missing(git, &project.upstream, &project.target, &released, &names)
        else {
            continue;
        };
        if missing.is_empty() {
            continue;
        }
        let uses = missing
            .iter()
            .map(|d| d.pack.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        checks.push(check(
            &format!("languages {}", project.name),
            CheckStatus::Warn,
            format!("uses {uses}, shown as plain text"),
            Some(format!("mg lang add --detect {}", project.name)),
        ));
    }
    checks
}

#[cfg(test)]
#[path = "languages_tests.rs"]
mod tests;
