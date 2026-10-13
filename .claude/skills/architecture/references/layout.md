# Layout and slices

## Layout

```
crates/
  core/                  domain: item state machine, revisions, events, triage policy,
                         inbox ordering, comment anchoring. No I/O dependencies.
  api/                   wire types (serde + generated TS)
  git/                   GitRepo port impl: gix reads + git-cli writes
  backend-github/
  backend-local/
  store-sqlite/
  highlight/             tree-sitter + WASM grammar loader + language manager
  judge-anthropic/  judge-openai/  judge-command/  judge-jev/
  auth-tailscale/  auth-token/
  hub/                   axum server, scheduler, hooks runner; composition root of the daemon
  cli/                   mg commands
  skill/                 mergegate's agent skill, embedded in the binary
ui/
  design-system/         tokens, primitives, keyboard map
  diff-view/             virtualized renderer; consumes pre-tokenized lines only
  screens/               inbox, review
```

`core` may not depend on I/O crates; a CI check fails the build if it does.

## Slices

| Slice | Responsibility | Port |
|---|---|---|
| Git operations | Staging repos, receive, test-merge, merge, push upstream | `GitRepo` |
| Backend | `github` / `local`: open/update/merge, external comment sync, polling | `Backend` |
| Diff engine | Merge-base diffs, interdiffs (range-diff semantics), viewed-state carry-over | in `core`, over `GitRepo` |
| Comment anchoring | Re-map drafts and old comments onto new revisions; flag unmappable ones | in `core` |
| File triage | Pipeline: guards → rules (only stage that can hide) → rankers (agent guide, structural heuristics, judge) | `FileRanker` |
| Inbox prioritization | Explicit priority → conflict pressure → age → optional judge | `ItemRanker` |
| Judge | Provider for both rankers; returns rank, reason, flags, confidence and `promote`, never a tier | `Judge` |
| Events & delivery | Event log, cursors, debounce, long-poll, `--wake`, hooks | in `hub` |
| Identity & transport | Listen modes, Tailscale whois, tokens | `Auth` |
| Highlighting | Language packs, tokenization, cache | `Highlighter` |
| Store | Schema, migrations, attachments | `Store` |
| Design system | Tokens, primitives, keyboard map | `ui/design-system` |
| Agent surface | CLI contract, skill, resume prompt | `cli`, `skill` |

## Plug-in points

- **Runtime, per project, via config:** backend, judge.
- **Build time, via cargo features:** optional adapters.
- **Out of process, via JSON over stdio:** judge `command` and hooks today, third-party backends later.

## Triage rules storage

Rules live in `~/.mergegate/projects/<p>/rules.toml`, ordered, first match wins.

- Proposals waiting for approval live in the database and are written to the file only once approved.
- The hub watches the file and validates hand edits. A hand edit that tightens (toward `review`) applies immediately.
- A hand edit that loosens (toward `trusted`) is held as a pending proposal until approved in the UI, because the hub can't tell the operator's edit from an agent's. Every rule in effect therefore has the operator's approval.
