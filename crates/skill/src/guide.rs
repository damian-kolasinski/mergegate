//! The mergegate agent skill's guide, embedded so `mg skill show` prints the
//! one that matches the binary and `mg skill install` writes it whole.

/// The skill's folder name, which is also its `name`.
pub const NAME: &str = "mergegate";

/// Every file in the skill folder, by path relative to it.
pub const FILES: &[(&str, &str)] = &[
    ("SKILL.md", include_str!("../mergegate/SKILL.md")),
    (
        "references/setup.md",
        include_str!("../mergegate/references/setup.md"),
    ),
    (
        "references/authoring.md",
        include_str!("../mergegate/references/authoring.md"),
    ),
    (
        "references/waiting.md",
        include_str!("../mergegate/references/waiting.md"),
    ),
    (
        "references/operator.md",
        include_str!("../mergegate/references/operator.md"),
    ),
    (
        "scripts/codex_wake.py",
        include_str!("../mergegate/scripts/codex_wake.py"),
    ),
];

/// The names `mg skill show NAME` takes, one per reference, in `FILES` order.
pub fn references() -> impl Iterator<Item = &'static str> {
    FILES[1..]
        .iter()
        .filter_map(|(path, _)| reference_name(path))
}

/// The guide `mg skill show` prints: `SKILL.md` without its frontmatter, or
/// with `name` the reference of that name; `None` for a name no reference
/// has. Links to references become the command that prints them, since the
/// folder an agent loads may hold only the stub.
pub fn show(name: Option<&str>) -> Option<String> {
    let text = match name {
        None => body(),
        Some(name) => {
            FILES[1..]
                .iter()
                .find(|(path, _)| reference_name(path) == Some(name))?
                .1
        }
    };
    Some(commands_for_links(text))
}

/// `SKILL.md` without its frontmatter, then each of the references it links
/// that `paths` names, under a `--- path ---` line, for prompts that inline
/// the skill. A last line names the references left out, which its links
/// still point at.
pub fn inlined(paths: &[&str]) -> String {
    let mut out = commands_for_links(body()).trim_end().to_owned();
    let mut left = Vec::new();
    for (path, text) in &FILES[1..] {
        if paths.contains(path) {
            let text = commands_for_links(text);
            out.push_str(&format!("\n\n--- {path} ---\n\n{}", text.trim_end()));
        } else {
            left.extend(reference_name(path));
        }
    }
    if !left.is_empty() {
        out.push_str(&format!(
            "\n\n--- Left out here: {}; `mg skill show NAME` prints each ---",
            left.join(", ")
        ));
    }
    out
}

/// `SKILL.md` without its frontmatter.
pub fn body() -> &'static str {
    frontmatter().map_or(FILES[0].1, |(_, body)| body.trim_start())
}

/// The `description` in `SKILL.md`'s frontmatter, which tells an agent when
/// to load the skill.
pub fn description() -> &'static str {
    frontmatter()
        .and_then(|(head, _)| {
            head.lines()
                .find_map(|line| line.strip_prefix("description: "))
        })
        .unwrap_or_default()
}

/// `SKILL.md`'s frontmatter, and what follows it.
fn frontmatter() -> Option<(&'static str, &'static str)> {
    FILES[0]
        .1
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
}

/// `waiting` for `references/waiting.md`.
fn reference_name(path: &str) -> Option<&str> {
    path.strip_prefix("references/")?.strip_suffix(".md")
}

/// Each `[x.md](x.md)` link to a reference, from `SKILL.md` or from another
/// reference, as `` `mg skill show x` ``. Every link in the skill names its
/// target as its text, which `every_link_names_its_target` checks.
fn commands_for_links(text: &str) -> String {
    let mut out = text.to_owned();
    for (path, _) in &FILES[1..] {
        let Some(name) = reference_name(path) else {
            continue;
        };
        let file = &path["references/".len()..];
        for target in [*path, file] {
            out = out.replace(
                &format!("[{target}]({target})"),
                &format!("`mg skill show {name}`"),
            );
        }
    }
    out
}

#[cfg(test)]
#[path = "guide_tests.rs"]
mod tests;
