//! Test harness: runs the real `mg` binary against real hubs, each with its own
//! temporary `MERGEGATE_HOME`, and returns what `mg` printed and its exit code.
//!
//! Integration tests of the `mg` package pass the binary in:
//! `Env::new(env!("CARGO_BIN_EXE_mg"))`.
//!
//! [`TestRepo`] builds the real git repositories those hubs work on, and
//! [`FakeGithub`] stands in for GitHub.

mod env;
mod files;
mod github;
mod http;
mod repo;

pub use env::{Env, Run};
pub use files::StaticFiles;
pub use github::{
    Comment as GithubComment, CommentKind as GithubCommentKind, FakeGithub, Inline as GithubInline,
    NewComment as GithubNewComment, Pull as GithubPull, PullState as GithubPullState,
};
pub use http::{Reply, http, http_reply};
pub use repo::{TestRepo, commit_files, git};
