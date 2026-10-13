//! Answers other than a plain result, in the envelopes `mg --json` prints as
//! they are: `{"error": {...}}` and `{"needs_decision": {...}}`.

use axum::Json;
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

/// Nothing changed; the client re-sends the request with the decision made.
pub(crate) fn decision(needs_decision: NeedsDecision) -> Response {
    (
        StatusCode::CONFLICT,
        Json(NeedsDecisionEnvelope { needs_decision }),
    )
        .into_response()
}
