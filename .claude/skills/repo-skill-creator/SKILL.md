---
name: repo-skill-creator
description: Create or update repository-local skills that must be available to both Claude and Codex. Use when adding, changing, or relocating a skill in this repository.
---

# Repository Skills

Keep each skill in one canonical location while exposing it to both supported agent harnesses:

- Store the real skill directory at `.claude/skills/<skill-name>` for Claude.
- Create `.agents/skills/<skill-name>` as a relative symlink to `../../.claude/skills/<skill-name>` for Codex.
- Add `.claude/skills/<skill-name>/agents/openai.yaml` with concise `display_name`, `short_description`, and `default_prompt` interface metadata so the skill renders well in Codex. The default prompt must mention `$<skill-name>`.
- Edit only the canonical directory; do not maintain copied skill directories for each harness.

Apply this layout to every repository-local skill, including this one.
