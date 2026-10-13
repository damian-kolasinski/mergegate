# Stack

## Drivers

- **The CLI and UI must be lightning fast.** Agents call `mg` constantly, so startup time and output size (tokens) matter. The UI must never wait on computation.
- **Agents maintain the codebase.** Choose for outcome quality and runtime speed, not human ergonomics. Strict compilers and enforced boundaries beat conventions.
- **Pluggable parts.** Every slice sits behind an interface and can be swapped or extended without touching the others.

## Decisions

Reversing any of these needs the operator's approval.

| Question | Decision | Why |
|---|---|---|
| Language | Rust, one binary for hub and CLI | ~1 ms startup, no GC, a strict compiler that catches mistakes early, native tree-sitter and gitoxide. |
| UI | Web UI served by the hub | Works on any OS and over the tailnet. A native shell (WKWebView) or PWA install can come later without a second UI. |
| Syntax highlighting | Server-side tree-sitter, cached per blob, optional per-language packs | Precomputed, so opening a file costs nothing; whole-file parsing is accurate; the parse also feeds triage. See `highlighting.md`. |
| Git | gitoxide for reads, the `git` binary for writes and anything that must match git exactly | See `git.md`. |
| Config format | TOML (`rules.toml`, `hub.toml`, project config) | YAML breaks on unquoted globs (`**/*.swift` parses as an alias), and Rust's YAML crates are unmaintained or forked. |
| Agent surface | Self-describing CLI + one skill embedded in the binary + `mg resume` prompt; no MCP server | See `agent-surface.md`. |

## Per layer

| Layer | Choice | Notes |
|---|---|---|
| Hub + CLI | Rust, single binary `mg` | `mg hub start` launches the daemon; every other command, `mg hub` admin commands included, is a client of it. |
| HTTP | axum + tokio | API, long-poll `mg wait`, SSE for the UI, git smart-HTTP. |
| CLI | clap; blocking HTTP client | Output contract (`--json`, exit codes, `needs_decision`) lives in one module. |
| Storage | SQLite, WAL (rusqlite) | Event log is an append-only table; its rowid is the `mg wait` cursor. Attachments stored on disk by content hash. |
| Git reads | gitoxide (`gix`) + imara-diff | Blob/tree reads, tree diffs, rev walks, line mapping for comment anchoring. |
| Git writes | `git` ≥ 2.38 subprocess | `http-backend` (receive), `merge-tree --write-tree`, push upstream, `updateInstead`. |
| GitHub | HTTP client with typed endpoints, ETag-conditional polling | Token from `gh auth token`. No octocrab. |
| Highlighting | tree-sitter, grammars loaded as WASM at runtime | See `highlighting.md`. |
| Tailscale | LocalAPI `whois` or `tailscale whois --json`, cached per peer | No Go client library in Rust. The macOS app exposes the LocalAPI differently from Linux, so the adapter does per-platform discovery. Which of the two to use on macOS is still open. |
| UI | SolidJS + TypeScript, esbuild, embedded in the binary | Fine-grained reactivity, no virtual DOM, ~7 KB runtime. |
| UI styling | Own design tokens (CSS custom properties) + small primitive set | No Tailwind, no component library. |
| API contract | Rust types are the source of truth; TS types generated (ts-rs or specta) | CLI, hub and UI cannot drift. |
| Packaging | cargo-dist → GitHub releases, Homebrew tap, curl installer | `mg hub start` installs a launchd / systemd user unit. |
