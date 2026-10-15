//! The operator's rules file, `rules.toml` in each project's folder: one
//! `[[rule]]` table per rule, first match wins.

use std::path::PathBuf;

use mergegate_core::project::ProjectName;
use mergegate_core::triage::{Kind, Rule, RuleId, RulesFile, RulesFileError, Tier};
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
    fn read(&self, project: &ProjectName) -> Result<Vec<Rule>, RulesFileError> {
        let path = self.path(project);
        match std::fs::read_to_string(&path) {
            Ok(text) => parse(&text).map_err(RulesFileError::Invalid),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(RulesFileError::Unreadable(format!(
                "reading {} failed: {e}",
                path.display()
            ))),
        }
    }

    fn location(&self, project: &ProjectName) -> String {
        self.path(project).display().to_string()
    }
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
