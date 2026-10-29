//! The mergegate agent skill, and setting agents up with it: a stub where
//! each agent loads skills, pointing at the guide this `mg` embeds, and a
//! session hook in those that run one.

mod agent;
mod folder;
mod guide;
mod hook;
mod json;
mod setup;
mod status;
mod stub;

pub use agent::Agent;
pub use guide::{FILES, NAME, body, description, inlined, references, show};
pub use setup::{Change, Kind, Step, refresh, remove, setup};
pub use status::{AgentStatus, Item, Scope, State, status};
pub use stub::is_ours;

/// Writes the whole skill folder, as `mg skill install` exports it.
pub fn export(path: &std::path::Path) -> std::io::Result<()> {
    folder::write(path, FILES)
}
