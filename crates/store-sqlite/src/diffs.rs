//! The `diffs` table: each blob pair's diff, its hunks kept as the body of a
//! unified diff.

use mergegate_core::diff::{Content, DiffStore, FileDiff, Hunk, Line, LineKind};
use mergegate_core::item::ItemId;
use mergegate_core::store::StoreError;
use rusqlite::{OptionalExtension, params};

use crate::Store;

/// Follows a line that doesn't end in a line break, as in git's output.
const NO_NEWLINE: &str = "\\ No newline at end of file";

impl DiffStore for Store {
    fn diff(&self, old: Option<&str>, new: Option<&str>) -> Result<Option<FileDiff>, StoreError> {
        let row = self
            .conn
            .prepare_cached(
                "SELECT kind, additions, deletions, hunks, lines_after FROM diffs
                 WHERE old_blob = ?1 AND new_blob = ?2",
            )
            .map_err(StoreError::new)?
            .query_row([old.unwrap_or_default(), new.unwrap_or_default()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            })
            .optional()
            .map_err(StoreError::new)?;
        let Some((kind, additions, deletions, hunks, lines_after)) = row else {
            return Ok(None);
        };
        let content = match kind.as_str() {
            "text" => Content::Text(decode(&hunks).map_err(StoreError::new)?),
            "binary" => Content::Binary,
            "too_large" => Content::TooLarge,
            _ => return Err(StoreError::new(format!("unknown diff kind {kind:?}"))),
        };
        Ok(Some(FileDiff {
            additions: additions.map(|n| n as u64),
            deletions: deletions.map(|n| n as u64),
            content,
            lines_after: lines_after.map(|n| n as u32),
        }))
    }

    fn insert_diff(
        &mut self,
        old: Option<&str>,
        new: Option<&str>,
        diff: &FileDiff,
    ) -> Result<(), StoreError> {
        let (kind, hunks) = match &diff.content {
            Content::Text(hunks) => ("text", encode(hunks)),
            Content::Binary => ("binary", String::new()),
            Content::TooLarge => ("too_large", String::new()),
        };
        self.conn
            .prepare_cached(
                "INSERT OR REPLACE INTO diffs
                     (old_blob, new_blob, kind, additions, deletions, hunks, lines_after)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .map_err(StoreError::new)?
            .execute(params![
                old.unwrap_or_default(),
                new.unwrap_or_default(),
                kind,
                diff.additions.map(|n| n as i64),
                diff.deletions.map(|n| n as i64),
                hunks,
                diff.lines_after,
            ])
            .map_err(StoreError::new)?;
        Ok(())
    }

    fn prune_diffs(&mut self, id: &ItemId) -> Result<(), StoreError> {
        // Every revision and interdiff of `id` counts, and every one of the
        // open items other than `id`, which the caller has already closed or
        // is closing.
        self.conn
            .prepare_cached(
                "WITH closing AS (
                     SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                     WHERE p.name = ?1 AND i.number = ?2
                 ),
                 kept AS (
                     SELECT i.id FROM items i
                     WHERE i.id <> (SELECT id FROM closing)
                         AND i.state IN ('needs_review', 'changes_requested', 'merging')
                 )
                 DELETE FROM diffs WHERE (old_blob, new_blob) IN (
                     SELECT coalesce(old_blob, ''), coalesce(new_blob, '')
                     FROM revision_files WHERE item_id = (SELECT id FROM closing)
                     UNION
                     SELECT coalesce(old_blob, ''), coalesce(new_blob, '')
                     FROM interdiff_files WHERE item_id = (SELECT id FROM closing)
                     EXCEPT
                     SELECT coalesce(old_blob, ''), coalesce(new_blob, '')
                     FROM revision_files WHERE item_id IN kept
                     EXCEPT
                     SELECT coalesce(old_blob, ''), coalesce(new_blob, '')
                     FROM interdiff_files WHERE item_id IN kept
                 )",
            )
            .map_err(StoreError::new)?
            .execute(params![id.project().as_str(), id.number() as i64])
            .map_err(StoreError::new)?;
        Ok(())
    }
}

/// Every header carries all four numbers, unlike git's, which drops a count
/// of 1.
fn encode(hunks: &[Hunk]) -> String {
    let mut out = String::new();
    for hunk in hunks {
        out += &format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
        );
        for line in &hunk.lines {
            out.push(match line.kind {
                LineKind::Context => ' ',
                LineKind::Added => '+',
                LineKind::Removed => '-',
            });
            out += &line.text;
            out.push('\n');
            if !line.newline {
                out += NO_NEWLINE;
                out.push('\n');
            }
        }
    }
    out
}

/// A line's text never holds a `\n`, and every line starts with a prefix, so
/// one that starts with `@@` is always a header. Splitting only on `\n` keeps
/// a `\r` the text ends with.
fn decode(text: &str) -> Result<Vec<Hunk>, String> {
    let mut hunks: Vec<Hunk> = Vec::new();
    for row in text.split_terminator('\n') {
        if let Some(header) = row.strip_prefix("@@ ") {
            hunks.push(header_of(header).ok_or_else(|| format!("bad hunk header {row:?}"))?);
            continue;
        }
        let lines = &mut hunks
            .last_mut()
            .ok_or_else(|| format!("line {row:?} before any hunk"))?
            .lines;
        if row == NO_NEWLINE {
            lines
                .last_mut()
                .ok_or_else(|| "no-newline marker before any line".to_string())?
                .newline = false;
            continue;
        }
        let kind = match row.as_bytes().first() {
            Some(b' ') => LineKind::Context,
            Some(b'+') => LineKind::Added,
            Some(b'-') => LineKind::Removed,
            _ => return Err(format!("bad diff line {row:?}")),
        };
        lines.push(Line {
            kind,
            text: row[1..].to_string(),
            newline: true,
        });
    }
    Ok(hunks)
}

/// `-a,b +c,d @@`
fn header_of(header: &str) -> Option<Hunk> {
    let (ranges, _) = header.split_once(" @@")?;
    let (old, new) = ranges.split_once(' ')?;
    let range = |range: &str, sign: char| -> Option<(u32, u32)> {
        let (start, lines) = range.strip_prefix(sign)?.split_once(',')?;
        Some((start.parse().ok()?, lines.parse().ok()?))
    };
    let (old_start, old_lines) = range(old, '-')?;
    let (new_start, new_lines) = range(new, '+')?;
    Some(Hunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        lines: Vec::new(),
    })
}

#[cfg(test)]
#[path = "diffs_tests.rs"]
mod tests;
