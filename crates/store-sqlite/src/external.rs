//! The `external_comments` and `event_comments` tables: comments from a
//! pull request, the batches that carry them, and the operator's override.

use mergegate_core::external::{Batch, Delivered, ExternalStore, Held, NewExternal, Override};
use mergegate_core::feedback::{Event, EventKind};
use mergegate_core::item::ItemId;
use mergegate_core::review::{Comment, CommentId, NewReply, Reply};
use mergegate_core::store::StoreError;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::comments::{self, CommentRow, FROM};
use crate::items::NOW;
use crate::{Store, replies};

/// `?1` and `?2` name the item.
const ITEM: &str = "SELECT i.id FROM items i JOIN projects p ON p.id = i.project_id
                    WHERE p.name = ?1 AND i.number = ?2";

impl ExternalStore for Store {
    fn insert_external(&mut self, comment: NewExternal, now: u64) -> Result<Comment, StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        let item_id = item_id(&tx, &comment.item)?;
        let anchor = comment.anchor.as_ref();
        let id: i64 = tx
            .query_row(
                &format!(
                    "INSERT INTO comments (item_id, revision, source, path, side, start_line,
                                           line, body, quote, created_at)
                     VALUES (?1, ?2, 'external', ?3, ?4, ?5, ?6, ?7, ?8, {NOW})
                     RETURNING id"
                ),
                params![
                    item_id,
                    comment.revision,
                    anchor.map(|a| &a.path),
                    anchor.map(|a| a.side.as_str()),
                    anchor.map(|a| a.start_line),
                    anchor.map(|a| a.line),
                    comment.body,
                    anchor.map(|a| &a.quote),
                ],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        tx.execute(
            "INSERT INTO external_comments
                 (comment_id, item_id, kind, github_id, login, bot, url, path, seen_at,
                  pending_at, dated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10)",
            params![
                id,
                item_id,
                comment.kind.as_str(),
                comment.github_id as i64,
                comment.login,
                comment.bot,
                comment.url,
                comment.path,
                now as i64,
                comment.dated as i64,
            ],
        )
        .map_err(StoreError::new)?;
        let inserted = comments::on_item(&tx, &comment.item, id)?;
        tx.commit().map_err(StoreError::new)?;
        Ok(inserted)
    }

    fn edit_external(
        &mut self,
        id: CommentId,
        body: &str,
        dated: u64,
        now: u64,
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        tx.execute(
            "UPDATE comments SET body = ?2 WHERE id = ?1 AND source = 'external'",
            params![id.0 as i64, body],
        )
        .map_err(StoreError::new)?;
        tx.execute(
            "UPDATE external_comments
             SET seen_at = ?2, pending_at = coalesce(pending_at, ?2),
                 dated_at = max(dated_at, ?3)
             WHERE comment_id = ?1",
            params![id.0 as i64, now as i64, dated as i64],
        )
        .map_err(StoreError::new)?;
        tx.commit().map_err(StoreError::new)
    }

    fn withdraw_external(&mut self, id: CommentId) -> Result<(), StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        let id = id.0 as i64;
        // A sent reply is in the agent's hands, or in a review's event, either
        // way referring to the comment.
        let kept: bool = tx
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM event_comments WHERE comment_id = ?1)
                     OR EXISTS (SELECT 1 FROM replies WHERE comment_id = ?1
                                AND (author = 'agent' OR review_id IS NOT NULL
                                     OR override = 1))",
                params![id],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        if kept {
            tx.execute(
                "UPDATE comments
                 SET body = coalesce((SELECT body FROM event_comments WHERE comment_id = ?1
                                      ORDER BY cursor DESC LIMIT 1), body)
                 WHERE id = ?1 AND source = 'external'",
                params![id],
            )
            .map_err(StoreError::new)?;
            tx.execute(
                "UPDATE external_comments SET pending_at = NULL WHERE comment_id = ?1",
                params![id],
            )
            .map_err(StoreError::new)?;
        } else {
            for sql in [
                "DELETE FROM replies WHERE comment_id = ?1",
                "DELETE FROM external_comments WHERE comment_id = ?1",
                "DELETE FROM comments WHERE id = ?1 AND source = 'external'",
            ] {
                tx.execute(sql, params![id]).map_err(StoreError::new)?;
            }
        }
        tx.commit().map_err(StoreError::new)
    }

    fn held(&self, id: &ItemId) -> Result<Option<Held>, StoreError> {
        let item = params![id.project().as_str(), id.number() as i64];
        let (first, last): (Option<i64>, Option<i64>) = self
            .conn
            .query_row(
                &format!(
                    "SELECT min(pending_at), max(seen_at) FROM external_comments
                     WHERE item_id = ({ITEM}) AND pending_at IS NOT NULL"
                ),
                item,
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(StoreError::new)?;
        let (Some(first), Some(last)) = (first, last) else {
            return Ok(None);
        };
        // The submit's stamp is this machine's clock and `dated_at` GitHub's,
        // which agree as far as NTP keeps them: far closer than the time a bot
        // takes to review a push.
        let pushed: i64 = self
            .conn
            .query_row(
                &format!(
                    "SELECT CAST(round(unixepoch(submitted_at, 'subsec') * 1000) AS INTEGER)
                     FROM revisions WHERE item_id = ({ITEM})
                     ORDER BY number DESC LIMIT 1"
                ),
                item,
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        // GitHub drops the fraction of a second, so a comment counted came at
        // or after the push. One in the push's second but after it counts as
        // before: the batch waits for the bot's next comment or the cap, which
        // is the side to err on, and no bot reviews a push that fast.
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "SELECT login FROM external_comments
                 WHERE item_id = ({ITEM}) AND dated_at >= ?3
                 GROUP BY login ORDER BY min(comment_id)"
            ))
            .map_err(StoreError::new)?;
        let heard = stmt
            .query_map(
                params![id.project().as_str(), id.number() as i64, pushed],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?
            .collect::<Result<_, _>>()
            .map_err(StoreError::new)?;
        Ok(Some(Held {
            first: first as u64,
            last: last as u64,
            pushed: pushed as u64,
            heard,
        }))
    }

    fn release(&mut self, id: &ItemId, revision: u32) -> Result<Option<Event>, StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        let item_id = item_id(&tx, id)?;
        let held: bool = tx
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM external_comments
                                WHERE item_id = ?1 AND pending_at IS NOT NULL)",
                params![item_id],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        if !held {
            return Ok(None);
        }
        let cursor: i64 = tx
            .query_row(
                "INSERT INTO events (item_id, type, revision, source)
                 VALUES (?1, 'external.comments', ?2, 'external') RETURNING cursor",
                params![item_id, revision],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        tx.execute(
            "INSERT INTO event_comments (cursor, comment_id, body, edited)
             SELECT ?2, c.id, c.body,
                    EXISTS (SELECT 1 FROM event_comments e
                            WHERE e.comment_id = c.id AND e.cursor < ?2)
             FROM comments c JOIN external_comments x ON x.comment_id = c.id
             WHERE x.item_id = ?1 AND x.pending_at IS NOT NULL",
            params![item_id, cursor],
        )
        .map_err(StoreError::new)?;
        tx.execute(
            "UPDATE external_comments SET pending_at = NULL
             WHERE item_id = ?1 AND pending_at IS NOT NULL",
            params![item_id],
        )
        .map_err(StoreError::new)?;
        let comments = batch(&tx, id, cursor as u64)?;
        tx.commit().map_err(StoreError::new)?;
        Ok(Some(Event {
            cursor: cursor as u64,
            kind: EventKind::ExternalComments(Batch { revision, comments }),
        }))
    }

    fn set_agreed(&mut self, id: CommentId, reply: u64) -> Result<(), StoreError> {
        self.conn
            .execute(
                "UPDATE external_comments SET agreed_reply = ?2 WHERE comment_id = ?1",
                params![id.0 as i64, reply as i64],
            )
            .map_err(StoreError::new)?;
        Ok(())
    }

    fn insert_override(&mut self, reply: NewReply) -> Result<(Reply, Event), StoreError> {
        let tx = self.conn.transaction().map_err(StoreError::new)?;
        let item_id = item_id(&tx, &reply.item)?;
        let revision = reply.revision;
        let reply = replies::insert_as(&tx, reply, true)?;
        let cursor: i64 = tx
            .query_row(
                "INSERT INTO events (item_id, type, revision, source, reply_id)
                 VALUES (?1, 'comment.override', ?2, 'operator', ?3) RETURNING cursor",
                params![item_id, revision, reply.id as i64],
                |row| row.get(0),
            )
            .map_err(StoreError::new)?;
        let kind = EventKind::Override(Box::new(overridden(&tx, reply.id)?));
        tx.commit().map_err(StoreError::new)?;
        let event = Event {
            cursor: cursor as u64,
            kind,
        };
        Ok((reply, event))
    }
}

/// The comments the `external.comments` event at `cursor` carried, with
/// their bodies as it did, oldest first.
pub(crate) fn batch(
    conn: &Connection,
    item: &ItemId,
    cursor: u64,
) -> Result<Vec<Delivered>, StoreError> {
    let mut stmt = conn
        .prepare_cached(&format!(
            "SELECT {}, e.edited FROM {FROM} JOIN event_comments e ON e.comment_id = c.id
             WHERE e.cursor = ?1 ORDER BY c.id",
            comments::columns("e.body")
        ))
        .map_err(StoreError::new)?;
    let rows = stmt
        .query_map(params![cursor as i64], |row| {
            Ok((CommentRow::read(row)?, row.get::<_, bool>(18)?))
        })
        .map_err(StoreError::new)?;
    rows.map(|row| {
        let (comment, edited) = row.map_err(StoreError::new)?;
        Ok(Delivered {
            comment: comment.into_comment(item)?,
            edited,
        })
    })
    .collect()
}

/// The override `reply` is, with the comment it's on.
pub(crate) fn overridden(conn: &Connection, reply: u64) -> Result<Override, StoreError> {
    let reply =
        replies::one(conn, reply)?.ok_or_else(|| StoreError::new(format!("no reply {reply}")))?;
    let comment = comments::one(conn, reply.comment)?
        .ok_or_else(|| StoreError::new(format!("no comment {}", reply.comment.0)))?;
    Ok(Override { comment, reply })
}

fn item_id(tx: &Transaction, id: &ItemId) -> Result<i64, StoreError> {
    tx.query_row(
        ITEM,
        params![id.project().as_str(), id.number() as i64],
        |row| row.get(0),
    )
    .optional()
    .map_err(StoreError::new)?
    .ok_or_else(|| StoreError::new(format!("no item {id}")))
}

#[cfg(test)]
#[path = "external_tests.rs"]
mod tests;
