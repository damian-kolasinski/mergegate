---
name: architecture
description: Apply mergegate's architecture rules (slices, ports and adapters, crate boundaries, git split, performance budgets) when adding or changing code, crates, adapters, dependencies, or UI structure. Use before deciding where code lives or what it may depend on.
---

# Architecture

mergegate is one Rust binary, `mg`, that is both the hub daemon and the CLI, with a SolidJS web UI embedded in it. Code is organized as ports and adapters: one crate per slice, with boundaries enforced by the crate graph. The guiding principle is **precompute at submit time, display at review time**.

Load the reference for the area you are changing:

| Reference | Covers |
|---|---|
| [references/stack.md](references/stack.md) | Technology per layer and the decisions behind it |
| [references/layout.md](references/layout.md) | Crate and UI layout, slices, ports, plug-in points, rules storage |
| [references/git.md](references/git.md) | gix vs the `git` binary, environment sanitization, interdiffs |
| [references/highlighting.md](references/highlighting.md) | Server-side tree-sitter, WASM language packs, `mg lang` |
| [references/agent-surface.md](references/agent-surface.md) | CLI contract, embedded skill, resume prompt |
| [references/quality.md](references/quality.md) | Performance budgets and testing |

If a change contradicts a reference, update the reference in the same change. If it reverses a decision in `references/stack.md`, stop and ask the operator first.

## Placement

- Domain logic goes in `crates/core`: item state machine, diff semantics, comment anchoring, triage policy, inbox ordering. `core` has no I/O: no async runtime, network, filesystem, subprocess, SQLite or git crates.
- `core` reaches every external system through a port trait: `GitRepo`, `Backend`, `Store`, `Highlighter`, `Judge`, `FileRanker`, `ItemRanker`, `Auth`, `Clock`. A new external system means a new port or an extension of an existing one.
- Adapters depend on `core` and `api` only, never on each other. Wiring happens only in the composition roots: `crates/hub` (daemon) and `crates/cli`.
- Wire types live in `crates/api`. TypeScript types are generated from them; never hand-write API types in `ui/`.
- Optional adapters sit behind cargo features; per-project choices (backend, judge) come from config.
- Third-party extension points run out of process (JSON over stdio), like judge `command` and hooks.

## Fixed rules

- **Only rules can hide files.** The agent guide, heuristics and the judge rank, annotate and promote files; they never assign `trusted`. Make the ranker and judge output types unable to express it: the judge returns `promote` (rule-trusted → `review`), never a tier.
- **Git split.** Reads (blobs, trees, history, diffs) go through `gix` and imara-diff. Receiving pushes, merges, test-merges and pushes go through the `git` binary with the sanitized environment in `references/git.md`. Never reimplement merging.
- **Precompute at submit, display at review.** Diffs and highlighting are cached by blob. Tiers are cached per revision and rules version, and recomputed for open items when rules change. UI request paths only read.
- **CLI contract.** Never prompt; every input is a flag. A missing decision exits 2 with `needs_decision`. `--json` everywhere, stable exit codes, compact output (agents pay per byte). Details in `references/agent-surface.md`.
- **Highlighting is hub-only.** Grammars are WASM language packs; a missing language renders as plain text.
- **Config is TOML.** Pending rule proposals live in the database. Loosening found in a hand edit of `rules.toml` is held for operator approval like any other proposal.
- **UI.** SolidJS + TypeScript. Styling uses the tokens and primitives in `ui/design-system`; no CSS frameworks or component libraries. `ui/diff-view` consumes pre-tokenized lines only.

## Dependencies

Choose for runtime speed and correctness, not human ergonomics. Before adding a crate or package, check that it is maintained, that it doesn't pull I/O into `core`, and that it doesn't add work to the CLI's startup path.

## Performance and tests

The budgets in `references/quality.md` are tests. A change on a budgeted path runs its benchmark, and a regression is a bug. Follow the testing approach in the same file.

## Before finishing

- The code lives in the slice that owns the behaviour.
- `core` gained no I/O dependency; no adapter imports another adapter.
- This skill and its references reflect any structural change.
