//! Re-mapping: following comments written on an earlier revision to the lines
//! they're about in a later one (F7, F13, design §9.2).

mod lines;
mod place;

pub use lines::{Mapped, map};
pub use place::{PlaceError, Placement, place};
