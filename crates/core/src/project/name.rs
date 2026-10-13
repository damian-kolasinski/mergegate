use std::fmt;

/// A project's name. It prefixes item IDs (`app#42`) and names the project's
/// directory in the hub's data, so it is short and plain: lowercase ASCII
/// letters, digits and inner dashes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectName(String);

pub const MAX_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error("a project name can't be empty")]
    Empty,
    #[error("project name {0:?} is longer than {MAX_LEN} characters")]
    TooLong(String),
    #[error(
        "project name {0:?} may only contain lowercase letters, digits and dashes, \
         and must start and end with a letter or digit"
    )]
    Invalid(String),
}

impl ProjectName {
    pub fn parse(name: &str) -> Result<Self, NameError> {
        if name.is_empty() {
            return Err(NameError::Empty);
        }
        if name.len() > MAX_LEN {
            return Err(NameError::TooLong(name.into()));
        }
        let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
        if !name.chars().all(allowed) || name.starts_with('-') || name.ends_with('-') {
            return Err(NameError::Invalid(name.into()));
        }
        Ok(Self(name.into()))
    }

    /// The name a repo directory suggests: lowercased, a `.git` suffix
    /// dropped, and every run of other characters turned into one dash.
    pub fn derive(dir_name: &str) -> Option<Self> {
        let stem = dir_name.strip_suffix(".git").unwrap_or(dir_name);
        let mut name = String::new();
        for c in stem.chars() {
            match c.to_ascii_lowercase() {
                c if c.is_ascii_lowercase() || c.is_ascii_digit() => name.push(c),
                _ if !name.is_empty() && !name.ends_with('-') => name.push('-'),
                _ => {}
            }
        }
        name.truncate(MAX_LEN);
        Self::parse(name.trim_end_matches('-')).ok()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProjectName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
#[path = "name_tests.rs"]
mod tests;
