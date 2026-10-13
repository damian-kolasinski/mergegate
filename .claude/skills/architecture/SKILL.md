---
name: architecture
description: Apply mergegate's architecture rules (slices, ports and adapters, crate boundaries, git split, performance budgets) when adding or changing code, crates, adapters, dependencies, or UI structure. Use before deciding where code lives or what it may depend on.
---

# Architecture

The source of truth is `docs/tech-stack.md` (stack, slices, budgets) and `docs/design.md` (behaviour). Read the sections a change touches before making it. If a change contradicts them, update the doc in the same change. If it reverses a decision recorded in `docs/tech-stack.md` §2, stop and ask the operator first.

## Placement

- Domain logic goes in `crates/core`: item state machine, diff semantics, comment anchoring, triage policy, inbox ordering. `core` has no I/O: no async runtime, network, filesystem, subprocess, SQLite or git crates.
- `core` reaches every external system through a port trait: `GitRepo`, `Backend`, `Store`, `Highlighter`, `Judge`, `FileRanker`, `ItemRanker`, `Auth`, `Clock`. A new external system means a new port or an extension of an existing one.
- Adapters depend on `core` and `api` only, never on each other. Wiring happens only in the composition roots: `crates/hub` (daemon) and `crates/cli`.
- Wire types live in `crates/api`. TypeScript types are generated from them; never hand-write API types in `ui/`.
- Optional adapters sit behind cargo features; per-project choices (backend, judge) come from config.
- Third-party extension points run out of process (JSON over stdio), like judge `command` and hooks.

## Fixed rules

- **Only rules can hide files.** The agent guide, heuristics and the judge rank, annotate and promote files; they never assign `trusted`. Make the ranker and judge output types unable to express it (promote-only, see `docs/design.md` §7.4).
- **Git split.** Reads (blobs, trees, history, diffs) go through `gix` and imara-diff. Receiving pushes, merges, test-merges and pushes go through the `git` binary with the sanitized environment in `docs/tech-stack.md` §5. Never reimplement merging.
- **Precompute at submit, display at review.** Diffs and highlighting are cached by blob. Tiers are cached per revision and rules version, and recomputed for open items when rules change. UI request paths only read.
- **CLI contract.** Never prompt; every input is a flag. A missing decision exits 2 with `needs_decision`. `--json` everywhere, stable exit codes, compact output (agents pay per byte).
- **Highlighting is hub-only.** Grammars are WASM language packs; a missing language renders as plain text.
- **Config is TOML.** Pending rule proposals live in the database. Loosening found in a hand edit of `rules.toml` is held for operator approval like any other proposal.
- **UI.** SolidJS + TypeScript. Styling uses the tokens and primitives in `ui/design-system`; no CSS frameworks or component libraries. `ui/diff-view` consumes pre-tokenized lines only.

## Dependencies

Choose for runtime speed and correctness, not human ergonomics. Before adding a crate or package, check that it is maintained, that it doesn't pull I/O into `core`, and that it doesn't add work to the CLI's startup path.

## Performance and tests

The budgets in `docs/tech-stack.md` §9 are tests. A change on a budgeted path runs its benchmark, and a regression is a bug. Test with real git repos in temp dirs, golden files for diffs, highlighting and triage, a fake GitHub server for the backend, and Playwright for the UI (§10).

## Before finishing

- The code lives in the slice that owns the behaviour.
- `core` gained no I/O dependency; no adapter imports another adapter.
- Docs reflect any structural change.
