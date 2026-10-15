//! `GET /api/items/{id}` and `/api/projects/{name}/blobs/{id}`: an item's
//! diff for review, with whatever spans the highlight pass has cached for
//! it, and the blobs the UI reads to show more of a file.

use std::sync::Arc;

use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use mergegate_core::attachment;
use mergegate_core::diff::{self, Content, LineKind};
use mergegate_core::highlight::{self, FileHighlights, Highlighter, Span};
use mergegate_core::item::{self, DiffedFile, ItemId};
use mergegate_core::triage::TriagedBy;

use crate::reply::Failure;
use crate::server::AppState;
use crate::{attachments, conversation, highlights, items, merge, projects, rules};

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
        let installed = state.languages.installed();
        let known = Arc::clone(&state);
        state
            .blocking(move |store, git| {
                let Some(diff) = item::item_diff(store, git, &id)? else {
                    return Ok(None);
                };
                let blocked = merge::blocked(store, &diff.item)?;
                let spans = diff
                    .files
                    .iter()
                    .map(|f| {
                        let hunks = match &f.content {
                            Some(Content::Text(hunks)) => hunks.as_slice(),
                            _ => &[],
                        };
                        let known = &known.known_packs;
                        highlight::file_highlights(store, &installed, known, &f.file, hunks)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let demo = attachment::demo(store, &id)?;
                Ok(Some((diff, blocked, spans, demo)))
            })
            .await
            .map_err(items::failure)?
    };
    let (diff, blocked, spans, demo) = found.ok_or_else(|| {
        Failure::new(
            StatusCode::NOT_FOUND,
            "item_not_found",
            format!("no item {id}"),
            Some("mg inbox".into()),
        )
    })?;
    Ok(Json(api::diff::ItemDiff {
        item: items::to_api(&diff.item),
        blocked,
        demo: demo.map(attachments::demo_to_api),
        files: diff.files.into_iter().zip(spans).map(file_to_api).collect(),
        guide: diff.guide,
        conversation: conversation::to_api(&diff.conversation),
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

fn file_to_api(
    (
        DiffedFile {
            file,
            additions,
            deletions,
            content,
            viewed,
            generated,
            triage,
            reason,
        },
        spans,
    ): (DiffedFile, FileHighlights),
) -> api::diff::File {
    let (rule, why) = match triage.by {
        None => (None, None),
        Some(TriagedBy::Guard(guard)) => (None, Some(guard.why().to_owned())),
        Some(TriagedBy::Rule { id, why }) => (Some(id.to_string()), Some(why)),
    };
    api::diff::File {
        path: file.path,
        old_path: file.old_path,
        old_blob: file.old_blob,
        new_blob: file.new_blob,
        additions,
        deletions,
        content: match content {
            Some(Content::Text(hunks)) => api::diff::Content::Text {
                hunks: hunks
                    .into_iter()
                    .map(|hunk| {
                        let lines = highlight::hunk_spans(&hunk, &spans);
                        hunk_to_api(&hunk, &lines)
                    })
                    .collect(),
            },
            Some(Content::Binary) => api::diff::Content::Binary,
            Some(Content::TooLarge) => api::diff::Content::TooLarge,
            None => api::diff::Content::Gone,
        },
        viewed,
        grammar: spans.grammar,
        missing_pack: spans.missing,
        generated,
        tier: rules::tier_to_api(triage.tier),
        rule,
        why,
        reason,
    }
}

/// `spans` are the spans of each of `hunk`'s lines.
fn hunk_to_api(hunk: &diff::Hunk, spans: &[&[Span]]) -> api::diff::Hunk {
    let mut lines = Vec::with_capacity(hunk.lines.len());
    let mut line_spans = Vec::with_capacity(hunk.lines.len());
    for (line, spans) in hunk.lines.iter().zip(spans) {
        let prefix = match line.kind {
            LineKind::Context => ' ',
            LineKind::Added => '+',
            LineKind::Removed => '-',
        };
        lines.push(format!("{prefix}{}", line.text));
        line_spans.push(highlights::spans_to_api(spans));
        if !line.newline {
            lines.push("\\ No newline at end of file".into());
            line_spans.push(Vec::new());
        }
    }
    api::diff::Hunk {
        old_start: hunk.old_start,
        old_lines: hunk.old_lines,
        new_start: hunk.new_start,
        new_lines: hunk.new_lines,
        lines,
        spans: line_spans,
    }
}
