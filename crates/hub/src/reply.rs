//! Answers other than a plain result, in the envelopes `mg --json` prints as
//! they are: `{"error": {...}}` and `{"needs_decision": {...}}`.

use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use api::decision::{NeedsDecision, NeedsDecisionEnvelope};
use api::error::{ErrorBody, ErrorEnvelope};

pub(crate) struct Failure {
    status: StatusCode,
    body: ErrorBody,
}

impl Failure {
    pub fn new(
        status: StatusCode,
        code: &str,
        message: impl Into<String>,
        hint: Option<String>,
    ) -> Self {
        Self {
            status,
            body: ErrorBody {
                code: code.into(),
                message: message.into(),
                hint,
            },
        }
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        (self.status, Json(ErrorEnvelope { error: self.body })).into_response()
    }
}

// axum answers malformed requests in plain text; handlers take their
// extractors as `Result`s and convert the rejection, so every answer the hub
// gives is in the envelope.
macro_rules! from_rejection {
    ($($rejection:ty),*) => {$(
        impl From<$rejection> for Failure {
            fn from(rejection: $rejection) -> Self {
                Self::new(rejection.status(), "invalid_request", rejection.body_text(), None)
            }
        }
    )*};
}

from_rejection!(JsonRejection, QueryRejection, PathRejection);

pub(crate) async fn no_route() -> Failure {
    Failure::new(StatusCode::NOT_FOUND, "not_found", "no such route", None)
}

pub(crate) async fn wrong_method() -> Failure {
    Failure::new(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "this route doesn't take that method",
        None,
    )
}

/// Nothing changed; the client re-sends the request with the decision made.
pub(crate) fn decision(needs_decision: NeedsDecision) -> Response {
    (
        StatusCode::CONFLICT,
        Json(NeedsDecisionEnvelope { needs_decision }),
    )
        .into_response()
}
