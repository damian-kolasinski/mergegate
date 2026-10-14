use std::fmt;

/// A repository on github.com, as `owner/name`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GithubRepo {
    owner: String,
    name: String,
}

impl GithubRepo {
    /// `owner/name`, as settings and `--github` spell it.
    pub fn parse(value: &str) -> Option<Self> {
        let (owner, name) = value.split_once('/')?;
        let name = name.strip_suffix(".git").unwrap_or(name);
        (valid_owner(owner) && valid_name(name)).then(|| Self {
            owner: owner.into(),
            name: name.into(),
        })
    }

    /// The repository a git remote URL points at, if it's on github.com:
    /// `git@github.com:o/r.git`, `ssh://git@github.com/o/r.git` or
    /// `https://github.com/o/r`, with or without `.git`.
    pub fn from_url(url: &str) -> Option<Self> {
        let path = url.strip_prefix("git@github.com:").or_else(|| {
            let rest = url
                .strip_prefix("https://")
                .or_else(|| url.strip_prefix("ssh://"))?;
            // Credentials or a user may come before the host.
            let rest = rest.rsplit_once('@').map_or(rest, |(_, host)| host);
            rest.strip_prefix("github.com/")
                .or_else(|| rest.strip_prefix("github.com:22/"))
        })?;
        Self::parse(path.trim_end_matches('/'))
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for GithubRepo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

/// GitHub's rule for user and organization names: alphanumerics and single
/// inner hyphens, at most 39 characters.
fn valid_owner(owner: &str) -> bool {
    (1..=39).contains(&owner.len())
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !owner.starts_with('-')
        && !owner.ends_with('-')
        && !owner.contains("--")
}

/// GitHub's rule for repository names: ASCII letters, digits, `.`, `-` and
/// `_`, at most 100, and never `.` or `..`.
fn valid_name(name: &str) -> bool {
    (1..=100).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        && name != "."
        && name != ".."
}

#[cfg(test)]
#[path = "repo_tests.rs"]
mod tests;
