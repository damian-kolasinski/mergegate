//! The `events` log and each item's delivered cursor.

use mergegate_core::external::Batch;
use mergegate_core::feedback::{Event, EventKind, FeedbackStore, Notice, NoticeKind, Source};
use mergegate_core::item::{ItemId, ItemState};
use mergegate_core::store::StoreError;
use rusqlite::{OptionalExtension, Row, params};

use crate::Store;
use crate::external::{batch, overridden};
use crate::reviews::{ReviewRow, review};

/// `?1` and `?2` name the item.
const ITEM: &str = "SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                    WHERE p.name = ?1 AND i.number = ?2";

impl FeedbackStore for Store {
    fn events(&self, id: &ItemId, after: u64) -> Result<Vec<Event>, StoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "SELECT e.cursor, e.type, e.revision, e.source, e.body, e.commit_id, e.paths,
                        r.id, r.revision, r.verdict, r.summary, r.submitted_at, e.reply_id
                 FROM events e LEFT JOIN reviews r ON r.id = e.review_id
                 WHERE e.item_id = ({ITEM}) AND e.cursor > ?3
                 ORDER BY e.cursor"
            ))
            .map_err(StoreError::new)?;
        let rows = stmt
            .query_map(
                params![id.project().as_str(), id.number() as i64, after as i64],
                EventRow::read,
            )
            .map_err(StoreError::new)?;
        rows.map(|row| {
            let mut row = row.map_err(StoreError::new)?;
            let cursor = row.cursor as u64;
            let kind = match (row.review.take(), row.reply) {
                (Some(found), _) => EventKind::ReviewSubmitted(review(&self.conn, id, found)?),
                (None, Some(reply)) => {
                    EventKind::Override(Box::new(overridden(&self.conn, reply as u64)?))
                }
                (None, None) if row.kind == "external.comments" => {
                    EventKind::ExternalComments(Batch {
                        revision: row.revision,
                        comments: batch(&self.conn, id, cursor)?,
                    })
                }
                (None, None) => EventKind::Notice(Notice {
                    revision: row.revision,
                    source: source_named(&row.source)?,
                    kind: notice_kind(row)?,
                }),
            };
            Ok(Event { cursor, kind })
        })
        .collect()
    }

    fn last_cursor(&self, id: &ItemId) -> Result<u64, StoreError> {
        let cursor: i64 = self
            .conn
            .query_row(
                &format!("SELECT coalesce(max(cursor), 0) FROM events WHERE item_id = ({ITEM})"),
                params![id.project().as_str(), id.number() as i64],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        Ok(cursor as u64)
    }

    fn delivered(&self, id: &ItemId) -> Result<u64, StoreError> {
        let cursor: Option<i64> = self
            .conn
            .query_row(
                "SELECT i.delivered FROM items i JOIN projects p ON p.id = i.project_id
                 WHERE p.name = ?1 AND i.number = ?2",
                params![id.project().as_str(), id.number() as i64],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::new)?;
        Ok(cursor.unwrap_or(0) as u64)
    }

    fn set_delivered(&mut self, id: &ItemId, cursor: u64) -> Result<(), StoreError> {
        self.conn
            .execute(
                &format!("UPDATE items SET delivered = ?3 WHERE id = ({ITEM})"),
                params![id.project().as_str(), id.number() as i64, cursor as i64],
            )
            .map_err(StoreError::new)?;
        Ok(())
    }

    fn notify(
        &mut self,
        id: &ItemId,
        state: ItemState,
        notice: Notice,
    ) -> Result<Event, StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        let item_id: i64 = tx
            .query_row(
                ITEM,
                params![id.project().as_str(), id.number() as i64],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::new)?
            .ok_or_else(|| StoreError::new(format!("no item {id}")))?;
        tx.execute(
            "UPDATE items SET state = ?2 WHERE id = ?1",
            params![item_id, state.as_str()],
        )
        .map_err(StoreError::new)?;
        let Columns {
            kind,
            body,
            commit_id,
            paths,
        } = columns(&notice.kind);
        let cursor: i64 = tx
            .query_row(
                "INSERT INTO events (item_id, type, revision, source, body, commit_id, paths)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING cursor",
                params![
                    item_id,
                    kind,
                    notice.revision,
                    notice.source.as_str(),
                    body,
                    commit_id,
                    paths
                ],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        tx.commit().map_err(StoreError::new)?;
        Ok(Event {
            cursor: cursor as u64,
            kind: EventKind::Notice(notice),
        })
    }
}

/// An `events` row as read; `review` is set for a review's event and `reply`
/// for an override's.
struct EventRow {
    cursor: i64,
    kind: String,
    revision: u32,
    source: String,
    body: Option<String>,
    commit_id: Option<String>,
    paths: Option<String>,
    review: Option<ReviewRow>,
    reply: Option<i64>,
}

impl EventRow {
    fn read(row: &Row) -> rusqlite::Result<Self> {
        let review = match row.get::<_, Option<i64>>(7)? {
            Some(id) => Some(ReviewRow {
                id,
                revision: row.get(8)?,
                verdict: row.get(9)?,
                summary: row.get(10)?,
                submitted_at: row.get(11)?,
            }),
            None => None,
        };
        Ok(Self {
            cursor: row.get(0)?,
            kind: row.get(1)?,
            revision: row.get(2)?,
            source: row.get(3)?,
            body: row.get(4)?,
            commit_id: row.get(5)?,
            paths: row.get(6)?,
            review,
            reply: row.get(12)?,
        })
    }
}

/// How a notice is spelled in its row.
struct Columns {
    kind: &'static str,
    body: Option<String>,
    commit_id: Option<String>,
    /// One per line, which git's conflict listing quotes out of any path.
    paths: Option<String>,
}

fn columns(kind: &NoticeKind) -> Columns {
    let spelled = |kind, body: Option<&String>| Columns {
        kind,
        body: body.cloned(),
        commit_id: None,
        paths: None,
    };
    match kind {
        NoticeKind::Abandoned { reason } => spelled("item.abandoned", reason.as_ref()),
        NoticeKind::MergeHeld { reason } => spelled("merge.held", Some(reason)),
        NoticeKind::MergeCancelled => spelled("merge.cancelled", None),
        NoticeKind::MergeConflict {
            target_commit,
            paths,
        } => Columns {
            commit_id: Some(target_commit.clone()),
            paths: Some(paths.join("\n")),
            ..spelled("merge.conflict", None)
        },
        NoticeKind::Merged { commit } => Columns {
            commit_id: Some(commit.clone()),
            ..spelled("item.merged", None)
        },
    }
}

fn notice_kind(row: EventRow) -> Result<NoticeKind, StoreError> {
    let kind = row.kind.as_str();
    let missing = |column| StoreError::new(format!("{kind} event without {column}"));
    match kind {
        "item.abandoned" => Ok(NoticeKind::Abandoned { reason: row.body }),
        "merge.held" => Ok(NoticeKind::MergeHeld {
            reason: row.body.ok_or_else(|| missing("body"))?,
        }),
        "merge.cancelled" => Ok(NoticeKind::MergeCancelled),
        "merge.conflict" => Ok(NoticeKind::MergeConflict {
            target_commit: row.commit_id.ok_or_else(|| missing("commit_id"))?,
            paths: match row.paths {
                Some(paths) if paths.is_empty() => Vec::new(),
                Some(paths) => paths.split('\n').map(Into::into).collect(),
                None => return Err(missing("paths")),
            },
        }),
        "item.merged" => Ok(NoticeKind::Merged {
            commit: row.commit_id.ok_or_else(|| missing("commit_id"))?,
        }),
        other => Err(StoreError::new(format!("unknown event type {other:?}"))),
    }
}

fn source_named(name: &str) -> Result<Source, StoreError> {
    Source::ALL
        .into_iter()
        .find(|s| s.as_str() == name)
        .ok_or_else(|| StoreError::new(format!("unknown source {name:?}")))
}

#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;
