/// Where an item is in its lifecycle (design §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemState {
    NeedsReview,
    ChangesRequested,
    Merging,
    Merged,
    Abandoned,
}

impl ItemState {
    pub const ALL: [Self; 5] = [
        Self::NeedsReview,
        Self::ChangesRequested,
        Self::Merging,
        Self::Merged,
        Self::Abandoned,
    ];

    /// The name the store spells the state with.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NeedsReview => "needs_review",
            Self::ChangesRequested => "changes_requested",
            Self::Merging => "merging",
            Self::Merged => "merged",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn is_open(self) -> bool {
        !matches!(self, Self::Merged | Self::Abandoned)
    }
}
