//! The `events` log and each item's delivered cursor.

use mergegate_core::feedback::{Event, EventKind, FeedbackStore, Notice, NoticeKind, Source};
use mergegate_core::item::{ItemId, ItemState};
use mergegate_core::store::StoreError;
use rusqlite::{OptionalExtension, Row, params};

use crate::Store;
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
                        r.id, r.revision, r.verdict, r.summary, r.submitted_at
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
            let row = row.map_err(StoreError::new)?;
            let cursor = row.cursor as u64;
            let kind = match row.review {
                Some(found) => EventKind::ReviewSubmitted(review(&self.conn, id, found)?),
                None => EventKind::Notice(Notice {
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
        let Columns { kind, body } = columns(&notice.kind);
        let cursor: i64 = tx
            .query_row(
                "INSERT INTO events (item_id, type, revision, source, body)
                 VALUES (?1, ?2, ?3, ?4, ?5) RETURNING cursor",
                params![item_id, kind, notice.revision, notice.source.as_str(), body],
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

/// An `events` row as read; `review` is set for a review's event.
struct EventRow {
    cursor: i64,
    kind: String,
    revision: u32,
    source: String,
    body: Option<String>,
    review: Option<ReviewRow>,
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
            review,
        })
    }
}

/// How a notice is spelled in its row.
struct Columns {
    kind: &'static str,
    body: Option<String>,
}

fn columns(kind: &NoticeKind) -> Columns {
    match kind {
        NoticeKind::Abandoned { reason } => Columns {
            kind: "item.abandoned",
            body: reason.clone(),
        },
    }
}

fn notice_kind(row: EventRow) -> Result<NoticeKind, StoreError> {
    match row.kind.as_str() {
        "item.abandoned" => Ok(NoticeKind::Abandoned { reason: row.body }),
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
