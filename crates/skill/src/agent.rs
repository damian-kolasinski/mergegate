//! The coding agents `mg skill setup` knows, and where each reads skills and
//! session hooks. Paths are relative to the scope's base: the user's home,
//! or a repository's root for `--project`, where every agent reads the same
//! relative paths.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Agent {
    Claude,
    Codex,
    OpenCode,
}

impl Agent {
    pub const ALL: [Agent; 3] = [Agent::Claude, Agent::Codex, Agent::OpenCode];

    /// As `--agent` takes it and the hook passes it to `session-start`.
    pub fn id(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::OpenCode => "opencode",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|agent| agent.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
            Agent::OpenCode => "OpenCode",
        }
    }

    /// Whether it is installed for the user whose home is `home`, by the
    /// folder it keeps its settings in.
    pub fn found(self, home: &Path) -> bool {
        let config = match self {
            Agent::Claude => ".claude",
            Agent::Codex => ".codex",
            Agent::OpenCode => ".config/opencode",
        };
        home.join(config).is_dir()
    }

    /// The folder it loads skills from. Codex and OpenCode share
    /// `.agents/skills`, which OpenCode reads beside `.claude/skills`.
    pub(crate) fn skills(self) -> &'static str {
        match self {
            Agent::Claude => ".claude/skills",
            Agent::Codex | Agent::OpenCode => ".agents/skills",
        }
    }

    /// Folders an older `mg` had the skill linked into for it. Codex lists
    /// both copies of a skill found twice, so setup removes ours there.
    pub(crate) fn former_skills(self) -> &'static [&'static str] {
        match self {
            Agent::Codex => &[".codex/skills"],
            Agent::Claude | Agent::OpenCode => &[],
        }
    }

    /// The file holding its command hooks; `None` when it runs none.
    pub(crate) fn hooks(self) -> Option<&'static str> {
        match self {
            Agent::Claude => Some(".claude/settings.json"),
            Agent::Codex => Some(".codex/hooks.json"),
            Agent::OpenCode => None,
        }
    }

    /// What the operator does before a new or changed hook runs, if
    /// anything: Codex runs one only once trusted, by its hash.
    pub fn hook_approval(self) -> Option<&'static str> {
        match self {
            Agent::Codex => Some("trust the new hook with /hooks in Codex"),
            Agent::Claude | Agent::OpenCode => None,
        }
    }
}
