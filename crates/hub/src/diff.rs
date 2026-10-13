//! `GET /api/items/{id}` and `/api/projects/{name}/blobs/{id}`: an item's
//! diff for review, and the blobs the UI reads to show more of a file.

use std::sync::Arc;

use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use mergegate_core::diff::{self, Content, FileDiff, LineKind};
use mergegate_core::item::{self, DiffedFile, ItemId};

use crate::reply::Failure;
use crate::server::AppState;
use crate::{items, projects};

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/items/{id}", get(show))
        .route("/api/projects/{name}/blobs/{id}", get(blob))
}

async fn show(
    State(state): State<Arc<AppState>>,
    id: Result<Path<String>, PathRejection>,
) -> Result<Json<api::diff::ItemDiff>, Failure> {
    let Path(id) = id?;
    let id = ItemId::parse(&id)
        .map_err(|e| Failure::new(StatusCode::BAD_REQUEST, "invalid_item", e.to_string(), None))?;
    let found = {
        let id = id.clone();
        state
            .blocking(move |store, _| item::item_diff(store, &id))
            .await
            .map_err(|e| items::failure(e.into()))?
    };
    let diff = found.ok_or_else(|| {
        Failure::new(
            StatusCode::NOT_FOUND,
            "item_not_found",
            format!("no item {id}"),
            Some("mg inbox".into()),
        )
    })?;
    Ok(Json(api::diff::ItemDiff {
        item: items::to_api(&diff.item),
        files: diff.files.into_iter().map(file_to_api).collect(),
    }))
}

/// Blobs are content-addressed, so a browser may keep one forever. `nosniff`
/// keeps a blob that looks like HTML from rendering on the hub's origin.
async fn blob(
    State(state): State<Arc<AppState>>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Result<impl IntoResponse, Failure> {
    let Path((name, id)) = path?;
    let found = {
        let id = id.clone();
        state
            .blocking(move |store, git| diff::blob(store, git, &name, &id))
            .await
            .map_err(projects::failure)?
    };
    let bytes = found.ok_or_else(|| {
        Failure::new(
            StatusCode::NOT_FOUND,
            "blob_not_found",
            format!("no blob {id}"),
            None,
        )
    })?;
    Ok((
        [
            (CONTENT_TYPE, "application/octet-stream"),
            (CACHE_CONTROL, "private, max-age=31536000, immutable"),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    ))
}

fn file_to_api(DiffedFile { file, diff }: DiffedFile) -> api::diff::File {
    let FileDiff {
        additions,
        deletions,
        content,
    } = diff;
    api::diff::File {
        path: file.path,
        old_path: file.old_path,
        old_blob: file.old_blob,
        new_blob: file.new_blob,
        additions,
        deletions,
        content: match content {
            Content::Text(hunks) => api::diff::Content::Text {
                hunks: hunks.into_iter().map(hunk_to_api).collect(),
            },
            Content::Binary => api::diff::Content::Binary,
            Content::TooLarge => api::diff::Content::TooLarge,
        },
    }
}

fn hunk_to_api(hunk: diff::Hunk) -> api::diff::Hunk {
    let mut lines = Vec::with_capacity(hunk.lines.len());
    for line in hunk.lines {
        let prefix = match line.kind {
            LineKind::Context => ' ',
            LineKind::Added => '+',
            LineKind::Removed => '-',
        };
        lines.push(format!("{prefix}{}", line.text));
        if !line.newline {
            lines.push("\\ No newline at end of file".into());
        }
    }
    api::diff::Hunk {
        old_start: hunk.old_start,
        old_lines: hunk.old_lines,
        new_start: hunk.new_start,
        new_lines: hunk.new_lines,
        lines,
    }
}
