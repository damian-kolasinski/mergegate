//! The `applied_rules` table, and the triage columns of `revision_files`.

use mergegate_core::item::ItemId;
use mergegate_core::project::ProjectName;
use mergegate_core::store::StoreError;
use mergegate_core::triage::{
    Guard, Kind, Rule, RuleId, Rules, RulesStore, Tier, Triage, TriagedBy,
};
use rusqlite::{Row, params};

use crate::Store;

impl RulesStore for Store {
    fn applied_rules(&self, project: &ProjectName) -> Result<Rules, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT r.id, r.tier, r.path, r.kind, r.max_lines, r.diff_matches, r.reason
                 FROM applied_rules r JOIN projects p ON p.id = r.project_id
                 WHERE p.name = ?1
                 ORDER BY r.position",
            )
            .map_err(StoreError::new)?;
        let rules = stmt
            .query_map([project.as_str()], read_rule)
            .map_err(StoreError::new)?
            .map(|row| row.map_err(StoreError::new)?)
            .collect::<Result<Vec<_>, _>>()?;
        Rules::new(rules).map_err(StoreError::new)
    }

    fn set_applied_rules(
        &mut self,
        project: &ProjectName,
        rules: &Rules,
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        let project_id: i64 = tx
            .query_row(
                "SELECT id FROM projects WHERE name = ?1",
                [project.as_str()],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        tx.execute(
            "DELETE FROM applied_rules WHERE project_id = ?1",
            [project_id],
        )
        .map_err(StoreError::new)?;
        {
            let mut stmt = tx
                .prepare_cached(
                    "INSERT INTO applied_rules
                         (project_id, position, id, tier, path, kind, max_lines, diff_matches,
                          reason)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                )
                .map_err(StoreError::new)?;
            for (position, rule) in rules.rules().enumerate() {
                stmt.execute(params![
                    project_id,
                    position as i64,
                    rule.id.number(),
                    rule.tier.as_str(),
                    rule.path,
                    rule.kind.map(Kind::as_str),
                    rule.max_lines.map(|n| n as i64),
                    rule.diff_matches,
                    rule.reason,
                ])
                .map_err(StoreError::new)?;
            }
        }
        tx.commit().map_err(StoreError::new)
    }

    fn set_triage(
        &mut self,
        id: &ItemId,
        revision: u32,
        triage: &[(String, Triage)],
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        {
            let mut stmt = tx
                .prepare_cached(
                    "UPDATE revision_files SET tier = ?4, rule = ?5, guard = ?6, why = ?7
                     WHERE revision = ?3 AND path = ?8 AND item_id = (
                         SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                         WHERE p.name = ?1 AND i.number = ?2)",
                )
                .map_err(StoreError::new)?;
            for (path, triage) in triage {
                let [tier, rule, guard, why] = columns(triage);
                stmt.execute(params![
                    id.project().as_str(),
                    id.number() as i64,
                    revision,
                    tier,
                    rule,
                    guard,
                    why,
                    path,
                ])
                .map_err(StoreError::new)?;
            }
        }
        tx.commit().map_err(StoreError::new)
    }
}

/// `tier`, `rule`, `guard` and `why`, as written.
pub(crate) fn columns(triage: &Triage) -> [rusqlite::types::Value; 4] {
    use rusqlite::types::Value;
    let text = |s: &str| Value::Text(s.to_owned());
    let (rule, guard, why) = match &triage.by {
        None => (Value::Null, Value::Null, Value::Null),
        Some(TriagedBy::Guard(guard)) => (Value::Null, text(guard.as_str()), Value::Null),
        Some(TriagedBy::Rule { id, why }) => {
            (Value::Integer(id.number().into()), Value::Null, text(why))
        }
    };
    [text(triage.tier.as_str()), rule, guard, why]
}

/// The triage in `row`'s columns from `first` on, in [`columns`]' order.
pub(crate) fn read_triage(row: &Row, first: usize) -> rusqlite::Result<Result<Triage, StoreError>> {
    let tier: String = row.get(first)?;
    let rule: Option<u32> = row.get(first + 1)?;
    let guard: Option<String> = row.get(first + 2)?;
    let why: Option<String> = row.get(first + 3)?;
    Ok((|| {
        let tier =
            Tier::parse(&tier).ok_or_else(|| StoreError::new(format!("unknown tier {tier:?}")))?;
        let by = match (rule, guard, why) {
            (None, None, None) => None,
            (None, Some(guard), None) => {
                Some(TriagedBy::Guard(Guard::parse(&guard).ok_or_else(|| {
                    StoreError::new(format!("unknown guard {guard:?}"))
                })?))
            }
            (Some(rule), None, Some(why)) => Some(TriagedBy::Rule {
                id: RuleId::new(rule),
                why,
            }),
            _ => return Err(StoreError::new("a file triaged by both a rule and a guard")),
        };
        Ok(Triage { tier, by })
    })())
}

fn read_rule(row: &Row) -> rusqlite::Result<Result<Rule, StoreError>> {
    let id: u32 = row.get(0)?;
    let tier: String = row.get(1)?;
    let kind: Option<String> = row.get(3)?;
    let (path, max_lines, diff_matches, reason) = (
        row.get(2)?,
        row.get::<_, Option<i64>>(4)?,
        row.get(5)?,
        row.get(6)?,
    );
    Ok((|| {
        let tier =
            Tier::parse(&tier).ok_or_else(|| StoreError::new(format!("unknown tier {tier:?}")))?;
        let kind = match kind {
            Some(kind) => Some(
                Kind::parse(&kind)
                    .ok_or_else(|| StoreError::new(format!("unknown kind {kind:?}")))?,
            ),
            None => None,
        };
        Ok(Rule {
            id: RuleId::new(id),
            tier,
            path,
            kind,
            max_lines: max_lines.map(|n| n as u64),
            diff_matches,
            reason,
        })
    })())
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
