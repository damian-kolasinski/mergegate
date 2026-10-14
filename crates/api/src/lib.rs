//! Wire types shared by the CLI, the hub and the UI.

pub mod decision;
pub mod diff;
pub mod doctor;
pub mod error;
pub mod event;
pub mod feedback;
pub mod highlight;
pub mod hub;
pub mod item;
pub mod lang;
pub mod merge;
pub mod project;
pub mod resume;
pub mod review;
pub mod viewed;

/// Version of the HTTP API between `mg` and the hub. A CLI refuses to talk to
/// a hub with a different API version, except through the endpoints every
/// version keeps: `GET /api/health` and `POST /api/hub/shutdown`.
pub const API_VERSION: u32 = 1;

/// Header every mutating request must carry. Browsers can't add it to a
/// cross-origin request without a CORS preflight, which the hub never grants.
pub const CLIENT_HEADER: &str = "x-mergegate-client";
