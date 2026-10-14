//! Domain logic of mergegate. No I/O: everything outside the process is
//! reached through the port traits defined here and implemented by adapters.

pub mod decision;
pub mod diff;
pub mod doctor;
#[cfg(test)]
mod fake;
pub mod git;
pub mod item;
pub mod project;
pub mod store;
pub mod viewed;
