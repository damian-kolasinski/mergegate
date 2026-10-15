//! Domain logic of mergegate. No I/O: everything outside the process is
//! reached through the port traits defined here and implemented by adapters.

pub mod agent;
pub mod attachment;
pub mod decision;
pub mod diff;
pub mod doctor;
pub mod external;
#[cfg(test)]
mod fake;
pub mod feedback;
pub mod git;
pub mod github;
pub mod highlight;
pub mod interdiff;
pub mod item;
pub mod merge;
pub mod project;
pub mod remap;
pub mod resume;
pub mod review;
pub mod settings;
pub mod store;
pub mod triage;
pub mod viewed;
