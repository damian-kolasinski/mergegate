---
name: architecture
description: Apply mergegate's architecture (crate layout, ports and adapters, dependency boundaries) when adding or changing code, crates, adapters, or dependencies. Use before deciding where code lives or what it may depend on.
---

# Architecture

mergegate is one Rust binary, `mg` (hub daemon and CLI), with a web UI embedded in it. Code is organized as ports and adapters, and the crate graph enforces the boundaries.

## Layout

```
crates/
  core/            domain logic and port traits; no I/O, no wire types
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
- `core` depends on no other workspace crate, `api` included, and has no serde: it doesn't know the wire format.
- `core` reaches every external system through a port trait it defines. Check the existing ports before adding one; a new external system means a new port or an extension of an existing one.
- Adapters depend on `core` and `api` only, never on each other. Wiring happens only in `hub` and `cli`.
- Wire types live in `api`. TypeScript types are generated from them; never hand-write API types in `ui/`.
- The edge maps domain to wire: `core` defines its own types, and the `hub` route module or `cli` command that serves them converts to and from `api`.
- Adapters are chosen per project through config, optional ones sit behind cargo features, and third-party extensions run out of process (JSON over stdio).
- Expensive work runs when data arrives and is cached; request paths only read.
- Build a new slice the way existing slices are built.

## Dependencies

Choose for runtime speed and correctness, not human ergonomics. Before adding a crate or package, check that it is maintained, that it doesn't pull I/O into `core`, and that it doesn't add work to the CLI's startup path.

## Before finishing

- The code lives in the slice that owns the behaviour.
- `core` gained no I/O dependency and no `api` import; no adapter imports another adapter. `crates/testkit/tests/boundaries.rs` checks the crate graph.
- This skill reflects any change to the layout or the rules above.
