/// What a changed file is, by its git mode. A diff reports only these four
/// for a file: git canonicalizes any other permissions to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileMode {
    Regular,
    Executable,
    Symlink,
    /// A gitlink: its object ID is a commit of another repository.
    Submodule,
}

impl FileMode {
    pub const ALL: [Self; 4] = [
        Self::Regular,
        Self::Executable,
        Self::Symlink,
        Self::Submodule,
    ];

    /// The octal mode, as git prints it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "100644",
            Self::Executable => "100755",
            Self::Symlink => "120000",
            Self::Submodule => "160000",
        }
    }

    pub fn parse(mode: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == mode)
    }
}

#[cfg(test)]
#[path = "mode_tests.rs"]
mod tests;
