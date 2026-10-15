//! The operator's rules file, `rules.toml` in each project's folder: one
//! `[[rule]]` table per rule, first match wins.

use std::path::{Path, PathBuf};

use mergegate_core::project::ProjectName;
use mergegate_core::triage::{
    Kind, Place, Rule, RuleId, RulesFile, RulesFileError, RulesText, Tier,
};
use serde::{Deserialize, Deserializer, de};

pub struct RulesToml {
    projects: PathBuf,
}

impl RulesToml {
    /// Each project's file is `<projects>/<name>/rules.toml`, next to its
    /// staging repo.
    pub fn new(projects: PathBuf) -> Self {
        Self { projects }
    }

    fn path(&self, project: &ProjectName) -> PathBuf {
        self.projects.join(project.as_str()).join("rules.toml")
    }
}

impl RulesFile for RulesToml {
    fn read(&self, project: &ProjectName) -> Result<Option<RulesText>, RulesFileError> {
        let path = self.path(project);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let rules = parse(&text).map_err(RulesFileError::Invalid)?;
                Ok(Some(RulesText { text, rules }))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(RulesFileError::Io(format!(
                "reading {} failed: {e}",
                path.display()
            ))),
        }
    }

    fn write(&self, project: &ProjectName, text: &str) -> Result<(), RulesFileError> {
        let path = self.path(project);
        let failed = |e: std::io::Error| {
            RulesFileError::Io(format!("writing {} failed: {e}", path.display()))
        };
        // Renamed into place, so the hub's next read never sees half a file.
        let temporary = path.with_extension("toml.tmp");
        std::fs::create_dir_all(self.projects.join(project.as_str())).map_err(failed)?;
        std::fs::write(&temporary, text).map_err(failed)?;
        std::fs::rename(&temporary, &path).map_err(failed)
    }

    fn insert(&self, text: &str, rule: &Rule, at: Place) -> Result<String, RulesFileError> {
        insert(text, rule, at).map_err(RulesFileError::Invalid)
    }

    fn location(&self, project: &ProjectName) -> String {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        tilde(&self.path(project), home.as_deref())
    }
}

/// `path` with the home folder written `~`, as the operator would type it.
fn tilde(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => Path::new("~").join(rest).display().to_string(),
        None => path.display().to_string(),
    }
}

/// `text` with `rule`'s table spliced in: first goes above the first
/// `[[rule]]` and the comment lines right over it, which describe that rule.
fn insert(text: &str, rule: &Rule, at: Place) -> Result<String, String> {
    let mut expected = parse(text)?;
    at.insert(&mut expected, rule.clone());
    let lines: Vec<&str> = text.lines().collect();
    let header = lines.iter().position(|line| {
        let line: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        line.starts_with("[[rule]]")
    });
    let block = render(rule);
    let spliced = match (at, header) {
        (Place::First, Some(mut first)) => {
            while first > 0 && lines[first - 1].trim_start().starts_with('#') {
                first -= 1;
            }
            let (before, after) = lines.split_at(first);
            let mut out: String = before.iter().map(|l| format!("{l}\n")).collect();
            out.push_str(&block);
            out.push('\n');
            out.extend(after.iter().map(|l| format!("{l}\n")));
            out
        }
        _ if text.trim().is_empty() => block,
        _ => format!("{}\n\n{block}", text.trim_end()),
    };
    // A `[[rule]]` line inside a multi-line string would put it wrong.
    if parse(&spliced).ok() != Some(expected) {
        return Err(format!(
            "couldn't add {} to the file without changing its other rules",
            rule.id
        ));
    }
    Ok(spliced)
}

fn render(rule: &Rule) -> String {
    let quoted = |s: &str| toml::Value::String(s.to_owned()).to_string();
    let mut out = format!(
        "[[rule]]\nid = \"{}\"\ntier = \"{}\"\n",
        rule.id,
        rule.tier.as_str()
    );
    if let Some(path) = &rule.path {
        out.push_str(&format!("path = {}\n", quoted(path)));
    }
    if let Some(kind) = rule.kind {
        out.push_str(&format!("kind = \"{}\"\n", kind.as_str()));
    }
    if let Some(max) = rule.max_lines {
        out.push_str(&format!("max_lines = {max}\n"));
    }
    if let Some(regex) = &rule.diff_matches {
        out.push_str(&format!("diff_matches = {}\n", quoted(regex)));
    }
    out.push_str(&format!("reason = {}\n", quoted(&rule.reason)));
    out
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    rule: Vec<TomlRule>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TomlRule {
    #[serde(deserialize_with = "id")]
    id: RuleId,
    #[serde(deserialize_with = "tier")]
    tier: Tier,
    path: Option<String>,
    #[serde(default, deserialize_with = "kind")]
    kind: Option<Kind>,
    max_lines: Option<u64>,
    diff_matches: Option<String>,
    reason: String,
}

/// The file's rules, or what's wrong with it as "line N: why".
fn parse(text: &str) -> Result<Vec<Rule>, String> {
    let file: File = toml::from_str(text).map_err(|e| {
        let message = e.message().trim_end();
        match e.span() {
            Some(span) => {
                let line = text[..span.start].matches('\n').count() + 1;
                format!("line {line}: {message}")
            }
            None => message.to_owned(),
        }
    })?;
    Ok(file
        .rule
        .into_iter()
        .map(|r| Rule {
            id: r.id,
            tier: r.tier,
            path: r.path,
            kind: r.kind,
            max_lines: r.max_lines,
            diff_matches: r.diff_matches,
            reason: r.reason,
        })
        .collect())
}

fn id<'de, D: Deserializer<'de>>(d: D) -> Result<RuleId, D::Error> {
    let id = String::deserialize(d)?;
    RuleId::parse(&id).ok_or_else(|| de::Error::custom(format!("{id:?} isn't an ID like R1")))
}

fn tier<'de, D: Deserializer<'de>>(d: D) -> Result<Tier, D::Error> {
    let tier = String::deserialize(d)?;
    Tier::parse(&tier).ok_or_else(|| {
        de::Error::custom(format!("unknown tier {tier:?}, expected review or trusted"))
    })
}

fn kind<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Kind>, D::Error> {
    let kind = String::deserialize(d)?;
    match Kind::parse(&kind) {
        Some(kind) => Ok(Some(kind)),
        None => {
            let kinds: Vec<_> = Kind::ALL.iter().map(|k| k.as_str()).collect();
            Err(de::Error::custom(format!(
                "unknown kind {kind:?}, expected one of {}",
                kinds.join(", ")
            )))
        }
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
