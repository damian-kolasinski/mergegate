//! Wire types shared by the CLI, the hub and the UI.

pub mod agent;
pub mod attachment;
pub mod bot;
pub mod decision;
pub mod device;
pub mod diff;
pub mod doctor;
pub mod error;
pub mod event;
pub mod feedback;
pub mod highlight;
pub mod hub;
pub mod item;
pub mod judge;
pub mod lang;
pub mod merge;
pub mod project;
pub mod resume;
pub mod review;
pub mod rules;
pub mod settings;
pub mod viewed;
pub mod weekly;

include!(concat!(env!("OUT_DIR"), "/api_version.rs"));

/// Version of the HTTP API between `mg` and the hub: a fingerprint of this
/// crate's code, which `build.rs` takes, so any change to a wire type changes
/// it. A CLI refuses to talk to a hub with a different API version, except
/// through the endpoints every version keeps: `GET /api/health` and
/// `POST /api/hub/shutdown`. Behaviour the hub changes without touching a
/// wire type keeps it; `mg hub status` and `mg doctor` warn of that hub by
/// its `mg` version instead.
pub const API_VERSION: u32 = FINGERPRINT;

/// Header every mutating request must carry. Browsers can't add it to a
/// cross-origin request without a CORS preflight, which the hub never grants.
pub const CLIENT_HEADER: &str = "x-mergegate-client";
