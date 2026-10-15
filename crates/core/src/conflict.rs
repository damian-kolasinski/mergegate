//! Early conflict detection (design §5, F9). Open items are test-merged into
//! their target whenever it moves, so a conflict reaches the agent before
//! the operator gets to the item, and into each other, so the inbox shows
//! which would conflict once the other lands.

mod error;
mod git;
mod pairs;
mod store;
mod watch;

pub use error::ConflictError;
pub use git::ConflictGit;
pub use pairs::{Pairing, conflicting, save_pairs};
pub use store::{ConflictStore, Pair};
pub use watch::{Merges, Seen, Watch};
