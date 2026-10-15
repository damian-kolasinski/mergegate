//! The `held_edits` and `held_rules` tables, with the preview in
//! `hidden_files`.

use mergegate_core::project::ProjectName;
use mergegate_core::store::StoreError;
use mergegate_core::triage::{HeldEdit, Preview};
use rusqlite::{Connection, OptionalExtension, params};

use super::{project_id, read_hidden, read_rules, used, write_hidden, write_rules};

pub(super) fn select(
    conn: &Connection,
    project: &ProjectName,
) -> Result<Option<HeldEdit>, StoreError> {
    let held: Option<(i64, String, u32)> = conn
        .query_row(
            "SELECT h.project_id, h.text, h.preview_items
             FROM held_edits h JOIN projects p ON p.id = h.project_id
             WHERE p.name = ?1",
            [project.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(StoreError::new)?;
    let Some((project_id, text, items)) = held else {
        return Ok(None);
    };
    Ok(Some(HeldEdit {
        rules: read_rules(conn, "held_rules", project)?,
        text,
        preview: Preview {
            items,
            hidden: read_hidden(conn, project, project_id, None)?,
        },
    }))
}

pub(super) fn set(
    conn: &mut Connection,
    project: &ProjectName,
    edit: Option<&HeldEdit>,
) -> Result<(), StoreError> {
    let tx = conn.transaction().map_err(StoreError::new)?;
    let project_id = project_id(&tx, project)?;
    tx.execute(
        "DELETE FROM hidden_files WHERE project_id = ?1 AND proposal IS NULL",
        [project_id],
    )
    .map_err(StoreError::new)?;
    // Its rules go with it.
    tx.execute("DELETE FROM held_edits WHERE project_id = ?1", [project_id])
        .map_err(StoreError::new)?;
    if let Some(edit) = edit {
        tx.execute(
            "INSERT INTO held_edits (project_id, text, preview_items) VALUES (?1, ?2, ?3)",
            params![project_id, edit.text, edit.preview.items],
        )
        .map_err(StoreError::new)?;
        write_rules(&tx, "held_rules", project_id, &edit.rules)?;
        write_hidden(&tx, project_id, None, &edit.preview.hidden)?;
        used(&tx, project_id, edit.rules.rules())?;
    }
    tx.commit().map_err(StoreError::new)
}
