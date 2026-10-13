# mergegate — tech stack & architecture

Status: draft v0.1 · 2026-10-13 · companion to [design.md](design.md)

## 1. Drivers

- **The CLI and UI must be lightning fast.** Agents call `mg` constantly, so startup time and output size (tokens) matter. The UI must never wait on computation.
- **Agents maintain the codebase.** We choose for outcome quality and runtime speed, not for human ergonomics. Strict compilers and enforced boundaries beat conventions written in a README.
- **Pluggable parts.** Every slice sits behind an interface and can be swapped or extended without touching the others.

The principle behind most choices below: **precompute at submit time, display at review time.** Diffs, tiers and highlighting are computed once per revision and cached by blob, so opening an item is a database read.

## 2. Decisions

| # | Question | Decision |
|---|---|---|
| 1 | Language | Rust. One binary for hub and CLI. |
| 2 | UI | Web UI served by the hub. A native shell (WKWebView) or PWA install can come later without a second UI. |
| 3 | Syntax highlighting | Server-side tree-sitter, cached per blob. Grammars are optional per-language downloads (§6). |
| 4 | Git | Split: gitoxide for reads, the `git` binary for writes and anything that must match git exactly (§5). |
| 5 | Config format | TOML (`rules.toml`, `hub.toml`, project config). YAML breaks on unquoted globs (`**/*.swift` is an alias) and Rust's YAML crates are unmaintained or forked. |
| 6 | Agent surface | Self-describing CLI + one skill embedded in the binary + `mg resume` prompt. No MCP server (§8). |

## 3. Stack

| Layer | Choice | Notes |
|---|---|---|
| Hub + CLI | Rust, single binary `mg` | ~1 ms startup, no GC. `mg hub …` runs the daemon, every other command is client mode. |
| HTTP | axum + tokio | API, long-poll `mg wait`, SSE for the UI, git smart-HTTP. |
| CLI | clap; blocking HTTP client | Output contract (`--json`, exit codes, `needs_decision`) lives in one module. |
| Storage | SQLite, WAL (rusqlite) | Event log is an append-only table; its rowid is the `mg wait` cursor. Attachments stored on disk by content hash. |
| Git reads | gitoxide (`gix`) + imara-diff | Blob/tree reads, tree diffs, rev walks, line mapping for comment anchoring. |
| Git writes | `git` ≥ 2.38 subprocess | `http-backend` (receive), `merge-tree --write-tree`, push upstream, `updateInstead`. Always run with a sanitized environment (no user/system config, no hooks). |
| GitHub | HTTP client with typed endpoints, ETag-conditional polling | Token from `gh auth token`. No octocrab. |
| Highlighting | tree-sitter, grammars loaded as WASM at runtime | See §6. |
| Tailscale | LocalAPI `whois` (or `tailscale whois --json`), cached per peer | No Go client library in Rust; the macOS app exposes the LocalAPI differently from Linux, so this is an adapter with per-platform discovery. |
| UI | SolidJS + TypeScript, esbuild, embedded in the binary | Fine-grained reactivity, no virtual DOM, ~7 KB runtime. |
| UI styling | Own design tokens (CSS custom properties) + small primitive set | No Tailwind, no component library. |
| API contract | Rust types are the source of truth; TS types generated (ts-rs or specta) | CLI, hub and UI cannot drift. |
| Packaging | cargo-dist → GitHub releases, Homebrew tap, curl installer | `mg hub start` installs a launchd / systemd user unit. |

## 4. Architecture: ports and adapters

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
  skill/                 SKILL.md + references, embedded
ui/
  design-system/         tokens, primitives, keyboard map
  diff-view/             virtualized renderer; consumes pre-tokenized lines only
  screens/               inbox, review
```

Ports (traits in `core`): `GitRepo`, `Backend`, `Store`, `Highlighter`, `Judge`, `FileRanker`, `ItemRanker`, `Auth`, `Clock`.

- **Agents follow these rules through the `architecture` skill** (`.claude/skills/architecture`).
- **Boundaries are enforced by the crate graph.** `core` may not depend on I/O crates; a CI check fails the build if it does. Adapters depend on `core`, never on each other.
- **Three ways to plug in:**
  - runtime, per project, via config: backend, judge;
  - build time via cargo features: optional adapters;
  - out of process via JSON over stdio: judge `command` and hooks today, third-party backends later.

## 5. Git split

| Operation | Implementation | Why |
|---|---|---|
| Receive pushes from workers | `git http-backend` (CGI) behind axum | gix has no server side. |
| Test-merge, merge | `git merge-tree --write-tree` | Must match git's `ort` merge exactly, or conflicts are detected wrongly. |
| Push to GitHub / operator repo | `git push` (`updateInstead` for local) | gix push is incomplete; must respect git semantics. |
| Read blobs, trees, history | gix | No process spawn per call (~2–5 ms each on macOS); typed results. |
| File diffs, interdiffs | gix tree diff + imara-diff (histogram) | In-memory; yields the line mapping that comment anchoring needs. |

Everything goes through the single `GitRepo` port, so any operation can move between gix and the `git` binary without callers changing. Subprocess calls run with `GIT_CONFIG_NOSYSTEM=1`, an isolated `HOME`, explicit `-c` overrides and hooks disabled, so the operator's personal git config can't change output.

**Interdiff after the base moved.** If the target advanced between r1 and r2, a raw r1→r2 diff includes upstream changes. The diff engine compares each revision against its own merge-base (range-diff semantics) and shows only the item's own changes.

## 6. Syntax highlighting and language packs

The hub parses the full old and new blob with tree-sitter at submit time. It stores per-line token spans (`[start, len, class]`) keyed by `(blob, language, grammar version)`. The UI receives text plus spans and paints CSS classes, so themes are free. The same parse feeds structural triage signals (§7).

**Languages are optional downloads, managed per hub.** Only the hub highlights, so workers never need grammars and `mg` itself ships with none.

A language pack contains:
- the grammar compiled to WASM: one file for every OS/arch, sandboxed, no native code loaded;
- the tree-sitter queries `highlights.scm` (plus `injections.scm` and `locals.scm` where present);
- a manifest entry: name, file extensions/filenames, version, tree-sitter ABI, sha256.

Grammars load through tree-sitter's WASM store (wasmtime), the same mechanism Zed uses for extension grammars. WASM parsing is slower than native, but it runs once per blob at submit time and is cached. Wasmtime is a fixed binary-size cost, paid once instead of per grammar.

Distribution and storage:
- packs are GitHub release assets of the mergegate repo, so there's no hosted infrastructure;
- each `mg` version pins the index URL and the checksums it accepts;
- packs are stored in `~/.mergegate/languages/<name>/<version>/`.

```
mg lang list [--available]              Installed languages (and what can be installed)
mg lang add <name>... | --detect [<project>]
mg lang add --from FILE                 Offline install of a pack file
mg lang remove <name>
mg lang update                          Refresh packs to versions compatible with this mg
```

- `mg project add` scans the repo's file extensions and returns detected languages in its `needs_decision`. The default answer is to install them.
- A file whose language isn't installed renders as plain text, with a hint in the UI ("No Swift highlighting · `mg lang add swift`").
- Adding a language highlights open items in the background. Removing one drops its cache entries.
- `mg doctor` flags packs whose ABI no longer matches after an `mg` upgrade.

## 7. Slices

| Slice | Responsibility | Port |
|---|---|---|
| Git operations | Staging repos, receive, test-merge, merge, push upstream | `GitRepo` |
| Backend | `github` / `local`: open/update/merge, external comment sync, polling | `Backend` |
| Diff engine | Merge-base diffs, interdiffs (range-diff semantics), viewed-state carry-over | in `core`, over `GitRepo` |
| Comment anchoring | Re-map drafts and old comments onto new revisions; flag unmappable ones | in `core` |
| File triage | Pipeline: guards → rules (only stage that can hide) → rankers (agent guide, structural heuristics, judge) | `FileRanker` |
| Inbox prioritization | Explicit priority → conflict pressure → age → optional judge | `ItemRanker` |
| Judge | Provider for both rankers | `Judge` |
| Events & delivery | Event log, cursors, debounce, long-poll, `--wake`, hooks | in `hub` |
| Identity & transport | Listen modes, Tailscale whois, tokens | `Auth` |
| Highlighting | Language packs, tokenization, cache | `Highlighter` |
| Store | Schema, migrations, attachments | `Store` |
| Design system | Tokens, primitives, keyboard map | `ui/design-system` |
| Agent surface | CLI contract, skill, resume prompt | `cli`, `skill` |

Rules are stored in `rules.toml`. Proposals waiting for approval live in the database and are written to the file only once approved, so everything in the file has the operator's approval. The hub watches the file and validates it after hand edits.

## 8. Agent surface

1. **Self-describing CLI.** Short `--help`, errors that name the next command, `needs_decision` with options. Output is compact by default, because every byte costs tokens.
2. **Skill embedded in the binary.** `mg skill install` writes the copy matching that binary, so the skill and the CLI never drift. A test asserts that every command the skill mentions exists.
3. **`mg resume`** prints a self-contained prompt for agents without skill support.

No MCP server. The CLI works in every harness, while MCP tool schemas cost context in every session.

## 9. Performance budgets (enforced in CI)

| Path | Budget |
|---|---|
| `mg` cold start (`mg --version`) | < 10 ms |
| `mg inbox --json` | < 30 ms |
| UI first paint | < 100 ms |
| Open item (precomputed) | < 50 ms |
| Switch file / keyboard action | < 16 ms |
| Scroll a 5k-line diff | 60 fps (virtualized) |

The UI also prefetches the next item and next file, and updates optimistically (marking viewed and similar actions don't wait for the server).

## 10. Testing

- Real git repos in temp dirs for git, diff, merge and anchoring tests.
- Golden files for diffs, interdiffs, highlighting spans and triage output.
- A fake GitHub server with recorded fixtures for the backend.
- Playwright end-to-end tests against a hub with a fixture database.
- Benchmarks for every budget in §9; a regression fails the build.

## 11. Open questions

- Should a tiny core set of languages (Markdown, JSON, TOML) be bundled, so highlighting works offline before any `mg lang add`?
- Per-platform Tailscale LocalAPI discovery on macOS: decide between the LocalAPI and the `tailscale` CLI after a spike.
