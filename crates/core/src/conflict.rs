//! Early conflict detection. Open items are test-merged into
//! their target whenever it moves, so a conflict reaches the agent before
//! the operator gets to the item.

mod error;
mod git;
mod watch;

pub use error::ConflictError;
pub use git::ConflictGit;
pub use watch::{Merges, Seen, Watch};
