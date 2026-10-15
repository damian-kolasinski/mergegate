//! The mergegate agent skill (design §11), embedded so `mg skill install`
//! writes the copy that matches the binary.

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
];

/// `SKILL.md` without its frontmatter, then each of the files it links that
/// `paths` names, under a `--- path ---` line, for prompts that inline the
/// skill. A last line names the files left out, which its links still point
/// at.
pub fn inlined(paths: &[&str]) -> String {
    let mut out = body().trim_end().to_owned();
    let mut left = Vec::new();
    for (path, text) in &FILES[1..] {
        if paths.contains(path) {
            out.push_str(&format!("\n\n--- {path} ---\n\n{}", text.trim_end()));
        } else {
            left.push(*path);
        }
    }
    if !left.is_empty() {
        out.push_str(&format!(
            "\n\n--- Left out here, in the installed skill: {} ---",
            left.join(", ")
        ));
    }
    out
}

/// `SKILL.md` without its frontmatter.
pub fn body() -> &'static str {
    let skill = FILES[0].1;
    skill
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map_or(skill, |(_, body)| body.trim_start())
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
