---
name: architecture
description: Apply mergegate's architecture (crate layout, ports and adapters, dependency boundaries) when adding or changing code, crates, adapters, or dependencies. Use before deciding where code lives or what it may depend on.
---

# Architecture

mergegate is one Rust binary, `mg` (hub daemon and CLI), with a web UI embedded in it. Code is organized as ports and adapters, and the crate graph enforces the boundaries.

## Layout

```
crates/
  core/            domain logic and port traits; no I/O
  api/             wire types shared by CLI, hub and UI
  <adapter>/       one crate per port implementation, e.g. backend-github, store-sqlite
  hub/             composition root of the daemon
  cli/             composition root of mg
  testkit/         test harness: drives the real hub, git and mg binaries
ui/
  design-system/   tokens and primitives; all styling goes through it
  <feature>/       UI features
```

## Rules

- Domain logic goes in `core`. `core` has no I/O: no async runtime, network, filesystem, subprocess, database or git crates.
- `core` reaches every external system through a port trait it defines. Check the existing ports before adding one; a new external system means a new port or an extension of an existing one.
- Adapters depend on `core` and `api` only, never on each other. Wiring happens only in `hub` and `cli`.
- Wire types live in `api`. TypeScript types are generated from them; never hand-write API types in `ui/`.
- Adapters are chosen per project through config, optional ones sit behind cargo features, and third-party extensions run out of process (JSON over stdio).
- Expensive work runs when data arrives and is cached; request paths only read.
- Build a new slice the way existing slices are built.

## Dependencies

Choose for runtime speed and correctness, not human ergonomics. Before adding a crate or package, check that it is maintained, that it doesn't pull I/O into `core`, and that it doesn't add work to the CLI's startup path.

Notes on dependency APIs live in `references/`, one file per dependency. Read the one you need before opening the crate's source; when you had to dig an API out of a crate's source, add what you found there.

- [gix](references/gix.md): reading repositories in the git adapter.

## Before finishing

- The code lives in the slice that owns the behaviour.
- `core` gained no I/O dependency; no adapter imports another adapter.
- This skill reflects any change to the layout or the rules above.
- A dependency API you had to dig out of its source is noted in `references/`.
