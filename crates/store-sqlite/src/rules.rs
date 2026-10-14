//! The rules tables: those applied, the changes waiting for the operator,
//! and the triage columns of `revision_files`.

mod held;
mod proposals;

use mergegate_core::item::ItemId;
use mergegate_core::project::ProjectName;
use mergegate_core::store::StoreError;
use mergegate_core::triage::{
    Guard, HeldEdit, Hidden, Kind, Proposal, ProposalState, Rule, RuleId, Rules, RulesStore, Tier,
    Triage, TriagedBy,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::Store;

impl RulesStore for Store {
    fn applied_rules(&self, project: &ProjectName) -> Result<Rules, StoreError> {
        read_rules(&self.conn, "applied_rules", project)
    }

    fn applied_text(&self, project: &ProjectName) -> Result<Option<String>, StoreError> {
        let text: Option<Option<String>> = self
            .conn
            .query_row(
                "SELECT f.applied_text FROM rules_files f JOIN projects p ON p.id = f.project_id
                 WHERE p.name = ?1",
                [project.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::new)?;
        Ok(text.flatten())
    }

    fn set_applied_rules(
        &mut self,
        project: &ProjectName,
        rules: &Rules,
        text: Option<&str>,
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        let project_id = project_id(&tx, project)?;
        write_rules(&tx, "applied_rules", project_id, rules)?;
        tx.execute(
            "INSERT INTO rules_files (project_id, applied_text, last_rule) VALUES (?1, ?2, 0)
             ON CONFLICT (project_id) DO UPDATE SET applied_text = excluded.applied_text",
            params![project_id, text],
        )
        .map_err(StoreError::new)?;
        used(&tx, project_id, rules.rules())?;
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

    fn next_rule_id(&self, project: &ProjectName) -> Result<RuleId, StoreError> {
        let last: Option<u32> = self
            .conn
            .query_row(
                "SELECT f.last_rule FROM rules_files f JOIN projects p ON p.id = f.project_id
                 WHERE p.name = ?1",
                [project.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::new)?;
        Ok(RuleId::new(last.unwrap_or(0) + 1))
    }

    fn insert_proposal(
        &mut self,
        project: &ProjectName,
        proposal: &Proposal,
    ) -> Result<(), StoreError> {
        proposals::insert(&mut self.conn, project, proposal)
    }

    fn proposal(&self, project: &ProjectName, id: RuleId) -> Result<Option<Proposal>, StoreError> {
        Ok(proposals::select(&self.conn, project, Some(id))?.pop())
    }

    fn pending_proposals(&self, project: &ProjectName) -> Result<Vec<Proposal>, StoreError> {
        let mut all = proposals::select(&self.conn, project, None)?;
        all.retain(|p| p.state == ProposalState::Pending);
        Ok(all)
    }

    fn set_proposal_state(
        &mut self,
        project: &ProjectName,
        id: RuleId,
        state: ProposalState,
    ) -> Result<(), StoreError> {
        proposals::set_state(&self.conn, project, id, state)
    }

    fn held_edit(&self, project: &ProjectName) -> Result<Option<HeldEdit>, StoreError> {
        held::select(&self.conn, project)
    }

    fn set_held_edit(
        &mut self,
        project: &ProjectName,
        edit: Option<&HeldEdit>,
    ) -> Result<(), StoreError> {
        held::set(&mut self.conn, project, edit)
    }
}

fn project_id(conn: &Connection, project: &ProjectName) -> Result<i64, StoreError> {
    conn.query_row(
        "SELECT id FROM projects WHERE name = ?1",
        [project.as_str()],
        |row| row.get(0),
    )
    .map_err(StoreError::new)
}

/// Raises the project's `last_rule` to cover `rules`.
fn used<'a>(
    conn: &Connection,
    project_id: i64,
    rules: impl IntoIterator<Item = &'a Rule>,
) -> Result<(), StoreError> {
    let Some(last) = rules.into_iter().map(|r| r.id.number()).max() else {
        return Ok(());
    };
    conn.execute(
        "INSERT INTO rules_files (project_id, last_rule) VALUES (?1, ?2)
         ON CONFLICT (project_id) DO UPDATE SET last_rule = max(last_rule, excluded.last_rule)",
        params![project_id, last],
    )
    .map_err(StoreError::new)?;
    Ok(())
}

/// `applied_rules` or `held_rules`, which share their columns.
fn read_rules(conn: &Connection, table: &str, project: &ProjectName) -> Result<Rules, StoreError> {
    let mut stmt = conn
        .prepare_cached(&format!(
            "SELECT r.id, r.tier, r.path, r.kind, r.max_lines, r.diff_matches, r.reason
             FROM {table} r JOIN projects p ON p.id = r.project_id
             WHERE p.name = ?1
             ORDER BY r.position"
        ))
        .map_err(StoreError::new)?;
    let rules = stmt
        .query_map([project.as_str()], read_rule)
        .map_err(StoreError::new)?
        .map(|row| row.map_err(StoreError::new)?)
        .collect::<Result<Vec<_>, _>>()?;
    Rules::new(rules).map_err(StoreError::new)
}

fn write_rules(
    conn: &Connection,
    table: &str,
    project_id: i64,
    rules: &Rules,
) -> Result<(), StoreError> {
    conn.execute(
        &format!("DELETE FROM {table} WHERE project_id = ?1"),
        [project_id],
    )
    .map_err(StoreError::new)?;
    let mut stmt = conn
        .prepare_cached(&format!(
            "INSERT INTO {table}
                 (project_id, position, id, tier, path, kind, max_lines, diff_matches, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
        ))
        .map_err(StoreError::new)?;
    for (position, rule) in rules.rules().enumerate() {
        let [id, tier, path, kind, max_lines, diff_matches, reason] = rule_columns(rule);
        stmt.execute(params![
            project_id,
            position as i64,
            id,
            tier,
            path,
            kind,
            max_lines,
            diff_matches,
            reason
        ])
        .map_err(StoreError::new)?;
    }
    Ok(())
}

/// The columns [`read_rule`] reads, in its order.
fn rule_columns(rule: &Rule) -> [rusqlite::types::Value; 7] {
    use rusqlite::types::Value;
    let text = |s: Option<&str>| s.map_or(Value::Null, |s| Value::Text(s.to_owned()));
    [
        Value::Integer(rule.id.number().into()),
        text(Some(rule.tier.as_str())),
        text(rule.path.as_deref()),
        text(rule.kind.map(Kind::as_str)),
        rule.max_lines
            .map_or(Value::Null, |n| Value::Integer(n as i64)),
        text(rule.diff_matches.as_deref()),
        text(Some(&rule.reason)),
    ]
}

/// A change's preview files: a proposal's, or the held edit's for none.
fn write_hidden(
    conn: &Connection,
    project_id: i64,
    proposal: Option<RuleId>,
    hidden: &[Hidden],
) -> Result<(), StoreError> {
    let mut stmt = conn
        .prepare_cached(
            "INSERT INTO hidden_files (project_id, proposal, item_id, path, rule)
             VALUES (?1, ?2, (SELECT id FROM items WHERE project_id = ?1 AND number = ?3), ?4,
                     ?5)",
        )
        .map_err(StoreError::new)?;
    for file in hidden {
        stmt.execute(params![
            project_id,
            proposal.map(RuleId::number),
            file.item.number() as i64,
            file.path,
            file.rule.number(),
        ])
        .map_err(StoreError::new)?;
    }
    Ok(())
}

fn read_hidden(
    conn: &Connection,
    project: &ProjectName,
    project_id: i64,
    proposal: Option<RuleId>,
) -> Result<Vec<Hidden>, StoreError> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT i.number, h.path, h.rule FROM hidden_files h JOIN items i ON i.id = h.item_id
             WHERE h.project_id = ?1 AND h.proposal IS ?2
             ORDER BY h.rowid",
        )
        .map_err(StoreError::new)?;
    stmt.query_map(params![project_id, proposal.map(RuleId::number)], |row| {
        Ok(Hidden {
            item: ItemId::new(project.clone(), row.get::<_, i64>(0)? as u64),
            path: row.get(1)?,
            rule: RuleId::new(row.get(2)?),
        })
    })
    .map_err(StoreError::new)?
    .collect::<rusqlite::Result<_>>()
    .map_err(StoreError::new)
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

/// A rule from `row`'s first seven columns, in [`rule_columns`]' order.
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
