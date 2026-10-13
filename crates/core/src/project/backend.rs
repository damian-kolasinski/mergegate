/// Where a project's approved work lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Pushes branches to GitHub, opens PRs and merges through the API.
    Github,
    /// Merges into the target branch of the operator's repo on the hub machine.
    Local,
}

impl Backend {
    pub const ALL: [Self; 2] = [Self::Github, Self::Local];

    /// The name settings and stored projects spell the backend with.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Local => "local",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.as_str() == name)
    }
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
