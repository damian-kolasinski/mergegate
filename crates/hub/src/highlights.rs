//! The background highlight pass, and `GET /api/highlights/{grammar}/{blob}`
//! reading what it cached. Spans are computed when an item's revision or the
//! installed packs change, never while a diff is being read.

use std::sync::Arc;

use api::event::UiEvent;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use mergegate_core::highlight::{self, HighlightStore, Highlighter, Highlights, Span, Tag};

use crate::items;
use crate::reply::Failure;
use crate::server::AppState;

pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/highlights/{grammar}/{blob}", get(show))
}

/// Passes at start, then each time something wakes it.
pub(crate) async fn run(state: Arc<AppState>) {
    loop {
        pass(&state).await;
        state.highlight.notified().await;
    }
}

async fn pass(state: &Arc<AppState>) {
    // Spans computed after the packs change land under this, older version,
    // and the pass that change wakes drops them.
    let installed = state.languages.installed();
    let pending = match state
        .blocking(move |store, _| highlight::pending(store, &installed))
        .await
    {
        Ok(pending) => pending,
        Err(e) => return tracing::error!("listing blobs to highlight: {e}"),
    };
    if pending.is_empty() {
        return;
    }
    for blob in pending {
        // Grammars run without the store, so requests aren't kept waiting.
        let languages = Arc::clone(&state.languages);
        let computed = state
            .blocking_git(move |git| {
                let spans = highlight::compute(git, &*languages, &blob);
                (blob, spans)
            })
            .await;
        let (blob, spans) = match computed {
            (blob, Ok(spans)) => (blob, spans),
            (blob, Err(e)) => {
                tracing::warn!("highlighting {} in {}: {e}", blob.blob, blob.project);
                continue;
            }
        };
        let stored = state
            .blocking(move |store, _| {
                store.insert_highlights(&blob.blob, &blob.grammar, &blob.version, &spans)
            })
            .await;
        if let Err(e) = stored {
            return tracing::error!("storing spans: {e}");
        }
    }
    state.events.send(UiEvent::HighlightsChanged);
}

async fn show(
    State(state): State<Arc<AppState>>,
    path: Result<Path<(String, String)>, PathRejection>,
) -> Result<Json<api::highlight::BlobHighlights>, Failure> {
    let Path((grammar, blob)) = path?;
    let installed = state.languages.installed();
    let found = {
        let (grammar, blob) = (grammar.clone(), blob.clone());
        state
            .blocking(move |store, _| {
                highlight::blob_highlights(store, &installed, &grammar, &blob)
            })
            .await
            .map_err(|e| items::failure(e.into()))?
    };
    let highlights = found.ok_or_else(|| {
        Failure::new(
            StatusCode::NOT_FOUND,
            "highlights_not_found",
            format!("no {grammar} spans for blob {blob}"),
            None,
        )
    })?;
    Ok(Json(to_api(&highlights)))
}

fn to_api(highlights: &Highlights) -> api::highlight::BlobHighlights {
    api::highlight::BlobHighlights {
        lines: highlights
            .lines
            .iter()
            .map(|line| spans_to_api(line))
            .collect(),
    }
}

pub(crate) fn spans_to_api(spans: &[Span]) -> Vec<api::highlight::Span> {
    spans
        .iter()
        .map(|s| api::highlight::Span(s.start, s.len, tag_to_api(s.tag)))
        .collect()
}

fn tag_to_api(tag: Tag) -> api::highlight::Tag {
    use api::highlight::Tag as T;
    match tag {
        Tag::Attribute => T::Attribute,
        Tag::Comment => T::Comment,
        Tag::Constant => T::Constant,
        Tag::Emphasis => T::Emphasis,
        Tag::Escape => T::Escape,
        Tag::Function => T::Function,
        Tag::Heading => T::Heading,
        Tag::Keyword => T::Keyword,
        Tag::Label => T::Label,
        Tag::Link => T::Link,
        Tag::Literal => T::Literal,
        Tag::Namespace => T::Namespace,
        Tag::Number => T::Number,
        Tag::Operator => T::Operator,
        Tag::Property => T::Property,
        Tag::Punctuation => T::Punctuation,
        Tag::String => T::String,
        Tag::Strong => T::Strong,
        Tag::Tag => T::Tag,
        Tag::Type => T::Type,
        Tag::Variable => T::Variable,
    }
}
