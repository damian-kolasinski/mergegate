//! Product issue reporting, independent of the review feedback workflow.

mod redact;

pub use redact::sanitize;

pub const REPOSITORY: &str = "damian-kolasinski/mergegate";
pub const MAX_TITLE: usize = 200;
pub const MAX_BODY: usize = 48 * 1024;
pub const MAX_LOGS: usize = 12 * 1024;

pub struct Report<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub body: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub number: u64,
    pub url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("{0}")]
    NotSent(String),
    #[error("{0}")]
    Uncertain(String),
}

pub trait IssueReporter {
    /// Lists may lag behind a successful create. Absence is never proof
    /// that an interrupted send did not create an issue.
    fn find(&self, id: &str) -> Result<Option<Issue>, ReportError>;
    fn create(&self, report: &Report<'_>) -> Result<Issue, ReportError>;
}

pub fn marker(id: &str) -> String {
    format!("<!-- mergegate-report:{id} -->")
}
