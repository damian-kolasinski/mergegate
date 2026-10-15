# mergegate

mergegate is a local, agent-operated code review hub: coding agents submit work through the `mg` CLI, the operator reviews it in a local UI with files ordered by importance, and feedback flows straight back to the agents, with GitHub or plain git underneath.

Commits and PRs carry only the operator's authorship: no AI co-author trailers, session links, or mentions of models, agents, or AI tools in commit messages or PR descriptions.

## Working

- Run checks with `.claude/scripts/check.sh` (usage at its top), not cargo directly.
- Ship work with `mg submit`, as the `mergegate` skill describes, not the `ship` skill: the hub opens the GitHub PR. Use the `ship` skill only when the hub is down or the operator asks for it.

## Code

- Organize by feature: one module per command, route group or table, so a new feature adds files instead of growing shared ones. Split a slice into layers only once it is too big on its own.
- No catch-all files (`utils`, `helpers`, `types`, one big router or `lib.rs`) that every change appends to. A module root declares its children and wires them; the work lives in them.
- Unit tests live in a file next to the code, never inline: `foo.rs` ends with `#[cfg(test)] #[path = "foo_tests.rs"] mod tests;`, so they stay a child module with access to private items. Tests through the public surface go in the crate's `tests/`.
- Comments say what the code can't: why, invariants, races, and a `SAFETY:` note on every `unsafe`. Don't restate the code.
