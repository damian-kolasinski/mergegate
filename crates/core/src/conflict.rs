//! Early conflict detection. Open items are test-merged into
//! their target whenever it moves, so a conflict reaches the agent before
//! the operator gets to the item, and the operator sees which merge cleanly
//! before approving them.

mod error;
mod git;
mod mergeable;
mod watch;

pub use error::ConflictError;
pub use git::ConflictGit;
pub use mergeable::{Clean, Mergeable, mergeable};
pub use watch::{Merges, Seen, Watch};
