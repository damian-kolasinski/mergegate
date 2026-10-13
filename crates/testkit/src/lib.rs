//! Test harness: runs the real `mg` binary against real hubs, each with its own
//! temporary `MERGEGATE_HOME`, and returns what `mg` printed and its exit code.
//!
//! Integration tests of the `mg` package pass the binary in:
//! `Env::new(env!("CARGO_BIN_EXE_mg"))`.
//!
//! [`TestRepo`] builds the real git repositories those hubs work on.

mod env;
mod http;
mod repo;

pub use env::{Env, Run};
pub use http::http;
pub use repo::{TestRepo, git};
