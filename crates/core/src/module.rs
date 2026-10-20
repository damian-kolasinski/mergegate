//! Modules: which part of the codebase each changed file belongs to, so the
//! operator sees a file in the module and folder it shapes. Found once per
//! revision from the folders around its files, and stored with them.

mod git;
mod locate;
mod place;

pub use git::{Entry, ModuleGit};
pub use locate::locate;
pub use place::{Placement, adopt};
