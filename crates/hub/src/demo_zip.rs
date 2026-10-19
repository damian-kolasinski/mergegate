//! `GET /api/items/{id}/demo.zip`: the demo the review screen shows, every
//! screenshot and recording in it as one download.

mod archive;

use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use axum::Router;
use axum::body::Body;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::{StreamExt, TryStreamExt, stream};
use mergegate_core::attachment::{self, AttachmentError};
use mergegate_core::item::ItemId;

use self::archive::{Entry, Modified, Part, TooLarge};
use crate::attachments;
use crate::reply::Failure;
use crate::server::AppState;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/items/{id}/demo.zip", get(download))
}

/// Where the hub serves `id`'s demo as a zip.
pub(crate) fn url(id: &ItemId) -> String {
    format!("/api/items/{}%23{}/demo.zip", id.project(), id.number())
}

/// A file of the demo, as the store keeps it.
struct Kept {
    name: String,
    size: u64,
    crc32: Option<u32>,
    path: PathBuf,
}

async fn download(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
) -> Result<Response, Failure> {
    let Path(id) = id?;
    let id = ItemId::parse(&id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let found = {
        let id = id.clone();
        state
            .blocking(move |store, _| {
                let Some(demo) = attachment::demo(store, &id)? else {
                    return Ok(None);
                };
                let names = archive::unique_names(demo.attachments.iter().map(|a| a.name.as_str()));
                let kept = demo
                    .attachments
                    .into_iter()
                    .zip(names)
                    .map(|(a, name)| Kept {
                        name,
                        size: a.size,
                        crc32: a.crc32,
                        path: store.attachment_file(&a.sha256),
                    })
                    .collect::<Vec<_>>();
                Ok(Some((demo.revision, kept)))
            })
            .await
            .map_err(|e| attachments::failure(AttachmentError::Store(e)))?
    };
    // An item with no demo and no item at all look alike from here: either
    // way there's nothing to download.
    let (revision, kept) = found.ok_or_else(|| {
        Failure::new(
            StatusCode::NOT_FOUND,
            "demo_not_found",
            format!("{id} has no demo"),
            None,
        )
    })?;
    // Files attached before the store kept their CRC are read for it, off
    // the async workers and the store's lock.
    let (entries, kept) = if kept.iter().any(|file| file.crc32.is_none()) {
        tokio::task::spawn_blocking(move || (entries(&kept), kept))
            .await
            .unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
    } else {
        (entries(&kept), kept)
    };
    let entries = entries.map_err(|e| match e.kind() {
        // Pruned since the lookup.
        std::io::ErrorKind::NotFound => Failure::new(
            StatusCode::NOT_FOUND,
            "demo_not_found",
            format!("{id}'s demo is gone"),
            None,
        ),
        _ => attachments::io_failure(e),
    })?;
    let modified = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (parts, len) =
        archive::layout(&entries, Modified::from_unix(modified)).map_err(|TooLarge| {
            Failure::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "demo_too_large",
                format!("{id}'s demo is too large for one zip; download its files one by one"),
                None,
            )
        })?;
    let files: Vec<(PathBuf, u64)> = kept.into_iter().map(|k| (k.path, k.size)).collect();
    let body = stream::iter(parts).flat_map(move |part| match part {
        Part::Bytes(bytes) => stream::once(async { Ok::<_, std::io::Error>(bytes) }).boxed(),
        Part::File(index) => {
            let (path, size) = files[index].clone();
            stream::once(tokio::fs::File::open(path))
                .map_ok(move |file| attachments::chunks(file, size))
                .try_flatten()
                .boxed()
        }
    });
    let mut response = Body::from_stream(body).into_response();
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    headers.insert(CONTENT_LENGTH, HeaderValue::from(len));
    // Project names are lowercase ASCII, so the file name needs no quoting.
    let file = format!(
        "attachment; filename=\"{}-{}-r{revision}-demo.zip\"",
        id.project(),
        id.number()
    );
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&file).expect("ASCII letters, digits and dashes"),
    );
    // A new revision's attachments change what this URL answers.
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

fn entries(kept: &[Kept]) -> std::io::Result<Vec<Entry>> {
    kept.iter()
        .map(|file| {
            Ok(Entry {
                name: file.name.clone(),
                size: file.size,
                crc32: match file.crc32 {
                    Some(crc32) => crc32,
                    None => checksum(file)?,
                },
            })
        })
        .collect()
}

fn checksum(file: &Kept) -> std::io::Result<u32> {
    let mut reader = std::fs::File::open(&file.path)?.take(file.size);
    let mut hasher = crc32fast::Hasher::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hasher.finalize())
}
