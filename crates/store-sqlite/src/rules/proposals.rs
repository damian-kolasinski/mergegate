//! The `rule_proposals` table, with each preview in `hidden_files`.

use mergegate_core::project::ProjectName;
use mergegate_core::store::StoreError;
use mergegate_core::triage::{Preview, Proposal, ProposalState, RuleId};
use rusqlite::{Connection, params};

use super::{project_id, read_hidden, read_rule, rule_columns, used, write_hidden};

pub(super) fn insert(
    conn: &mut Connection,
    project: &ProjectName,
    proposal: &Proposal,
) -> Result<(), StoreError> {
    let tx = conn.transaction().map_err(StoreError::new)?;
    let project_id = project_id(&tx, project)?;
    let [id, tier, path, kind, max_lines, diff_matches, reason] = rule_columns(&proposal.rule);
    tx.execute(
        "INSERT INTO rule_proposals
             (project_id, id, tier, path, kind, max_lines, diff_matches, reason, state,
              preview_items)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            project_id,
            id,
            tier,
            path,
            kind,
            max_lines,
            diff_matches,
            reason,
            proposal.state.as_str(),
            proposal.preview.as_ref().map(|p| p.items),
        ],
    )
    .map_err(StoreError::new)?;
    if let Some(preview) = &proposal.preview {
        write_hidden(&tx, project_id, Some(proposal.rule.id), &preview.hidden)?;
    }
    used(&tx, project_id, [&proposal.rule])?;
    tx.commit().map_err(StoreError::new)
}

/// `id`'s, or every one in ID order for none.
pub(super) fn select(
    conn: &Connection,
    project: &ProjectName,
    id: Option<RuleId>,
) -> Result<Vec<Proposal>, StoreError> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT r.id, r.tier, r.path, r.kind, r.max_lines, r.diff_matches, r.reason,
                    r.state, r.preview_items, r.project_id
             FROM rule_proposals r JOIN projects p ON p.id = r.project_id
             WHERE p.name = ?1 AND (?2 IS NULL OR r.id = ?2)
             ORDER BY r.id",
        )
        .map_err(StoreError::new)?;
    let rows = stmt
        .query_map(params![project.as_str(), id.map(RuleId::number)], |row| {
            let state: String = row.get(7)?;
            let items: Option<u32> = row.get(8)?;
            let project_id: i64 = row.get(9)?;
            Ok((read_rule(row)?, state, items, project_id))
        })
        .map_err(StoreError::new)?;
    let mut proposals = Vec::new();
    for row in rows {
        let (rule, state, items, project_id) = row.map_err(StoreError::new)?;
        let rule = rule?;
        let state = ProposalState::parse(&state)
            .ok_or_else(|| StoreError::new(format!("unknown proposal state {state:?}")))?;
        let preview = match items {
            Some(items) => Some(Preview {
                items,
                hidden: read_hidden(conn, project, project_id, Some(rule.id))?,
            }),
            None => None,
        };
        proposals.push(Proposal {
            rule,
            state,
            preview,
        });
    }
    Ok(proposals)
}

pub(super) fn set_state(
    conn: &Connection,
    project: &ProjectName,
    id: RuleId,
    state: ProposalState,
) -> Result<(), StoreError> {
    let changed = conn
        .execute(
            "UPDATE rule_proposals SET state = ?3
             WHERE id = ?2 AND project_id = (SELECT id FROM projects WHERE name = ?1)",
            params![project.as_str(), id.number(), state.as_str()],
        )
        .map_err(StoreError::new)?;
    if changed == 0 {
        return Err(StoreError::new(format!("{project} has no proposal {id}")));
    }
    Ok(())
}
