use crate::project::ProjectName;

/// Closures in the hub's local calendar week, Monday through Sunday.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WeeklyClosures {
    pub monday: String,
    pub today: String,
    pub repositories: Vec<RepositoryClosures>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryClosures {
    pub project: ProjectName,
    /// Merged and abandoned items, indexed from Monday (0).
    pub days: [u32; 7],
}
