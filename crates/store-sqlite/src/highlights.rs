//! The `highlights` table: each blob's spans under a grammar and packs version.

use mergegate_core::highlight::{HighlightStore, Highlights, Span, Tag};
use mergegate_core::item::ItemId;
use mergegate_core::store::StoreError;
use rusqlite::{OptionalExtension, params};

use crate::Store;

impl HighlightStore for Store {
    fn highlights(
        &self,
        blob: &str,
        grammar: &str,
        version: &str,
    ) -> Result<Option<Highlights>, StoreError> {
        let lines: Option<String> = self
            .conn
            .prepare_cached(
                "SELECT lines FROM highlights WHERE blob = ?1 AND grammar = ?2 AND version = ?3",
            )
            .map_err(StoreError::new)?
            .query_row([blob, grammar, version], |row| row.get(0))
            .optional()
            .map_err(StoreError::new)?;
        lines
            .map(|lines| decode(&lines).map_err(StoreError::new))
            .transpose()
    }

    fn highlight_lines(
        &self,
        blob: &str,
        grammar: &str,
        version: &str,
        lines: &[u32],
    ) -> Result<Option<Highlights>, StoreError> {
        let text: Option<String> = self
            .conn
            .prepare_cached(
                "SELECT lines FROM highlights WHERE blob = ?1 AND grammar = ?2 AND version = ?3",
            )
            .map_err(StoreError::new)?
            .query_row([blob, grammar, version], |row| row.get(0))
            .optional()
            .map_err(StoreError::new)?;
        text.map(|text| decode_lines(&text, lines).map_err(StoreError::new))
            .transpose()
    }

    fn has_highlights(&self, blob: &str, grammar: &str, version: &str) -> Result<bool, StoreError> {
        self.conn
            .prepare_cached(
                "SELECT EXISTS (SELECT 1 FROM highlights
                 WHERE blob = ?1 AND grammar = ?2 AND version = ?3)",
            )
            .map_err(StoreError::new)?
            .query_row([blob, grammar, version], |row| row.get(0))
            .map_err(StoreError::new)
    }

    fn insert_highlights(
        &mut self,
        blob: &str,
        grammar: &str,
        version: &str,
        highlights: &Highlights,
    ) -> Result<(), StoreError> {
        self.conn
            .prepare_cached(
                "INSERT OR REPLACE INTO highlights (blob, grammar, version, lines)
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .map_err(StoreError::new)?
            .execute(params![blob, grammar, version, encode(highlights)])
            .map_err(StoreError::new)?;
        Ok(())
    }

    fn retain_highlights(&mut self, version: &str) -> Result<(), StoreError> {
        self.conn
            .prepare_cached("DELETE FROM highlights WHERE version <> ?1")
            .map_err(StoreError::new)?
            .execute([version])
            .map_err(StoreError::new)?;
        Ok(())
    }

    fn prune_highlights(&mut self, id: &ItemId) -> Result<(), StoreError> {
        // As prune_diffs: every revision of `id`, against every revision of
        // the open items other than `id`.
        self.conn
            .prepare_cached(
                "WITH closing AS (
                     SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                     WHERE p.name = ?1 AND i.number = ?2
                 ),
                 open_files AS (
                     SELECT f.old_blob, f.new_blob
                     FROM revision_files f JOIN items i ON i.id = f.item_id
                     WHERE i.id <> (SELECT id FROM closing)
                         AND i.state IN ('needs_review', 'changes_requested', 'merging')
                 )
                 DELETE FROM highlights WHERE blob IN (
                     SELECT old_blob FROM revision_files
                     WHERE item_id = (SELECT id FROM closing) AND old_blob IS NOT NULL
                     UNION
                     SELECT new_blob FROM revision_files
                     WHERE item_id = (SELECT id FROM closing) AND new_blob IS NOT NULL
                     EXCEPT
                     SELECT old_blob FROM open_files
                     EXCEPT
                     SELECT new_blob FROM open_files
                 )",
            )
            .map_err(StoreError::new)?
            .execute(params![id.project().as_str(), id.number() as i64])
            .map_err(StoreError::new)?;
        Ok(())
    }
}

fn encode(highlights: &Highlights) -> String {
    let mut out = String::new();
    for line in &highlights.lines {
        for (i, span) in line.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out += &format!("{},{},{}", span.start, span.len, span.tag.name());
        }
        out.push('\n');
    }
    out
}

fn decode(text: &str) -> Result<Highlights, String> {
    let lines = text
        .split_terminator('\n')
        .map(decode_line)
        .collect::<Result<_, _>>()?;
    Ok(Highlights { lines })
}

/// Only `wanted` (from 1, ascending) parsed, the lines before them left
/// empty and those after left out.
fn decode_lines(text: &str, wanted: &[u32]) -> Result<Highlights, String> {
    let mut lines = Vec::new();
    let mut wanted = wanted.iter().copied().peekable();
    for (number, line) in (1..).zip(text.split_terminator('\n')) {
        let Some(&next) = wanted.peek() else { break };
        if next == number {
            lines.push(decode_line(line)?);
            while wanted.next_if_eq(&number).is_some() {}
        } else {
            lines.push(Vec::new());
        }
    }
    Ok(Highlights { lines })
}

fn decode_line(line: &str) -> Result<Vec<Span>, String> {
    let span = |field: &str| -> Option<Span> {
        let mut parts = field.split(',');
        let span = Span {
            start: parts.next()?.parse().ok()?,
            len: parts.next()?.parse().ok()?,
            tag: Tag::parse(parts.next()?)?,
        };
        parts.next().is_none().then_some(span)
    };
    line.split_terminator(' ')
        .map(|field| span(field).ok_or_else(|| format!("bad span {field:?}")))
        .collect()
}

#[cfg(test)]
#[path = "highlights_tests.rs"]
mod tests;
