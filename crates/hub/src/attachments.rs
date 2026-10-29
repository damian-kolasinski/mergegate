//! Demo attachments: `POST /api/items/{id}/attachments` takes a screenshot or
//! recording as the raw body, `GET /api/attachments/{sha256}` serves one, and
//! the pruner drops what closed items attached once the hub's
//! `attachment-ttl` is up.

use std::io::SeekFrom;
use std::sync::Arc;
use std::time::Duration;

use api::event::UiEvent;
use axum::Json;
use axum::Router;
use axum::body::Body;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::header::{
    ACCEPT_RANGES, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_SECURITY_POLICY,
    CONTENT_TYPE, HeaderMap, RANGE, X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::{Stream, StreamExt, stream};
use mergegate_core::attachment::{
    self, Attach, Attachment, AttachmentError, AttachmentStore, MAX_SIZE,
};
use mergegate_core::item::ItemId;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::demo_zip;
use crate::reply::Failure;
use crate::server::AppState;

/// How often the pruner looks without being asked; it's also asked when an
/// item closes and when the TTL changes.
const PRUNE_EVERY: Duration = Duration::from_secs(3600);

const CHUNK: usize = 64 * 1024;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/items/{id}/attachments", post(upload))
        .route("/api/attachments/{sha256}", get(serve))
}

async fn upload(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
    query: Result<Query<api::attachment::AttachTo>, QueryRejection>,
    body: Body,
) -> Result<Json<api::attachment::Attached>, Failure> {
    let Path(id) = id?;
    let Query(query) = query?;
    let id = ItemId::parse(&id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let bytes = read(body, &query.name).await?;
    let attached = {
        let id = id.clone();
        state
            .blocking(move |store, _| {
                let request = Attach {
                    id: &id,
                    revision: query.revision,
                    name: &query.name,
                    bytes: &bytes,
                };
                attachment::attach(store, request)
            })
            .await
            .map_err(failure)?
    };
    // Nothing for the agent to read and no state to move: only what the
    // review screen and the inbox show.
    state.events.send(UiEvent::InboxChanged);
    state.events.send(UiEvent::ItemChanged {
        item: id.to_string(),
    });
    Ok(Json(api::attachment::Attached {
        item: id.to_string(),
        attachment: to_api(attached),
    }))
}

/// Reads an upload up to [`MAX_SIZE`]. A handler that takes the raw `Body`
/// isn't held to axum's default limit, so this is the only one.
async fn read(body: Body, name: &str) -> Result<Vec<u8>, Failure> {
    let mut chunks = body.into_data_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|e| {
            Failure::new(
                StatusCode::BAD_REQUEST,
                "upload_failed",
                format!("receiving {name} failed: {e}"),
                None,
            )
        })?;
        if (bytes.len() + chunk.len()) as u64 > MAX_SIZE {
            return Err(failure(AttachmentError::TooLarge {
                name: name.to_owned(),
                size: MAX_SIZE + 1,
            }));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Attachments are named by their content, so a browser may keep one
/// forever. The media type is the one sniffed on upload; `nosniff` and the
/// sandbox keep a file from ever running as a page on the hub's origin.
async fn serve(
    State(state): State<Arc<AppState>>,
    sha256: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
) -> Result<Response, Failure> {
    let Path(sha256) = sha256?;
    let not_found = || {
        Failure::new(
            StatusCode::NOT_FOUND,
            "attachment_not_found",
            format!("no attachment {sha256}"),
            None,
        )
    };
    if sha256.len() != 64
        || !sha256
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(not_found());
    }
    let found = {
        let sha256 = sha256.clone();
        state
            .blocking(move |store, _| {
                let media = store.attachment_media(&sha256)?;
                Ok(media.map(|media| (media, store.attachment_file(&sha256))))
            })
            .await
            .map_err(|e| failure(AttachmentError::Store(e)))?
    };
    let (media, path) = found.ok_or_else(not_found)?;
    let mut file = match tokio::fs::File::open(&path).await {
        Ok(file) => file,
        // Pruned between the lookup and the open.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(not_found()),
        Err(e) => return Err(io_failure(e)),
    };
    let len = file.metadata().await.map_err(io_failure)?.len();
    let range = headers
        .get(RANGE)
        .and_then(|v| v.to_str().ok())
        .map(|v| range(v, len));
    let (status, start, end) = match range {
        None | Some(Range::Ignored) => (StatusCode::OK, 0, len),
        Some(Range::Bytes { start, end }) => (StatusCode::PARTIAL_CONTENT, start, end),
        Some(Range::Unsatisfiable) => {
            let mut response = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
            response
                .headers_mut()
                .insert(CONTENT_RANGE, header_value(&format!("bytes */{len}")));
            return Ok(response);
        }
    };
    if start > 0 {
        file.seek(SeekFrom::Start(start))
            .await
            .map_err(io_failure)?;
    }
    let body = chunks(file, end - start);
    let mut response = (status, Body::from_stream(body)).into_response();
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(media.mime()));
    headers.insert(CONTENT_LENGTH, header_value(&(end - start).to_string()));
    headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox"));
    if status == StatusCode::PARTIAL_CONTENT {
        headers.insert(
            CONTENT_RANGE,
            header_value(&format!("bytes {start}-{}/{len}", end - 1)),
        );
    }
    Ok(response)
}

/// The next `len` bytes of `file`; an error if it ends sooner.
pub(crate) fn chunks(
    file: tokio::fs::File,
    len: u64,
) -> impl Stream<Item = std::io::Result<Vec<u8>>> + Send + 'static {
    stream::try_unfold((file, len), |(mut file, left)| async move {
        if left == 0 {
            return Ok(None);
        }
        let mut chunk = vec![0; CHUNK.min(left as usize)];
        let n = file.read(&mut chunk).await?;
        if n == 0 {
            return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
        }
        chunk.truncate(n);
        Ok(Some((chunk, (file, left - n as u64))))
    })
}

#[derive(Debug, PartialEq, Eq)]
enum Range {
    /// `start..end`, end exclusive.
    Bytes {
        start: u64,
        end: u64,
    },
    Unsatisfiable,
    /// Not one byte range: answered with the whole file, which RFC 9110
    /// allows for any range a server doesn't take.
    Ignored,
}

/// Reads a `Range` header against a file of `len` bytes. Players ask for one
/// range at a time, so a list of them is ignored.
fn range(header: &str, len: u64) -> Range {
    let Some(spec) = header.trim().strip_prefix("bytes=") else {
        return Range::Ignored;
    };
    let Some((first, last)) = spec.trim().split_once('-') else {
        return Range::Ignored;
    };
    if spec.contains(',') {
        return Range::Ignored;
    }
    let number = |s: &str| s.trim().parse::<u64>().ok();
    let (start, end) = match (first.trim(), last.trim()) {
        ("", "") => return Range::Ignored,
        ("", suffix) => match number(suffix) {
            Some(0) => return Range::Unsatisfiable,
            Some(n) => (len.saturating_sub(n), len),
            None => return Range::Ignored,
        },
        (first, "") => match number(first) {
            Some(start) => (start, len),
            None => return Range::Ignored,
        },
        (first, last) => match (number(first), number(last)) {
            (Some(start), Some(last)) if start <= last => (start, last.saturating_add(1).min(len)),
            _ => return Range::Ignored,
        },
    };
    if start >= len {
        return Range::Unsatisfiable;
    }
    Range::Bytes { start, end }
}

/// Runs a prune now, then each hour and whenever [`AppState::prune`] is
/// notified, for as long as the hub runs.
pub(crate) async fn prune(state: Arc<AppState>) {
    loop {
        match state.blocking(|store, _| attachment::prune(store)).await {
            Ok(items) if items.is_empty() => {}
            Ok(items) => {
                tracing::info!(
                    event_code = "attachments_pruned",
                    count = items.len() as u64,
                    "pruned the attachments of {} closed items",
                    items.len()
                );
                state.events.send(UiEvent::InboxChanged);
                // An open review of one would show a strip of files now gone.
                for id in items {
                    state.events.send(UiEvent::ItemChanged {
                        item: id.to_string(),
                    });
                }
            }
            Err(e) => tracing::error!("pruning attachments failed: {e}"),
        }
        tokio::select! {
            () = state.prune.notified() => {}
            () = tokio::time::sleep(PRUNE_EVERY) => {}
        }
    }
}

pub(crate) fn demo_to_api(id: &ItemId, demo: attachment::Demo) -> api::attachment::Demo {
    api::attachment::Demo {
        revision: demo.revision,
        zip_url: demo_zip::url(id),
        attachments: demo.attachments.into_iter().map(to_api).collect(),
    }
}

fn to_api(attachment: Attachment) -> api::attachment::Attachment {
    let Attachment {
        revision,
        name,
        media,
        size,
        sha256,
        crc32: _,
    } = attachment;
    api::attachment::Attachment {
        revision,
        name,
        media_type: media.mime().into(),
        video: media.is_video(),
        size,
        url: format!("/api/attachments/{sha256}"),
    }
}

fn header_value(value: &str) -> HeaderValue {
    HeaderValue::from_str(value).expect("digits, spaces, a dash and a slash")
}

pub(crate) fn io_failure(e: std::io::Error) -> Failure {
    tracing::error!("reading an attachment failed: {e}");
    Failure::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "attachment_unreadable",
        format!("reading the attachment failed: {e}"),
        None,
    )
}

pub(crate) fn failure(e: AttachmentError) -> Failure {
    use AttachmentError as E;
    use StatusCode as S;
    let (status, code, hint) = match &e {
        E::NoItem(_) => (S::NOT_FOUND, "item_not_found", Some("mg inbox".to_owned())),
        E::Closed(_) => (S::CONFLICT, "item_closed", None),
        E::NoRevision { .. } => (S::NOT_FOUND, "revision_not_found", None),
        E::InvalidName(_) => (S::BAD_REQUEST, "invalid_name", None),
        E::TooLarge { .. } => (S::PAYLOAD_TOO_LARGE, "attachment_too_large", None),
        E::NotMedia(_) => (S::UNSUPPORTED_MEDIA_TYPE, "unsupported_media", None),
        E::Store(_) => (S::INTERNAL_SERVER_ERROR, "store_failed", None),
    };
    if status.is_server_error() {
        tracing::error!("{e}");
    }
    Failure::new(status, code, e.to_string(), hint)
}

#[cfg(test)]
#[path = "attachments_tests.rs"]
mod tests;
