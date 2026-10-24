# iOS Mobile

Agreed direction for implementing the client one feature at a time; this is a decision record, not a feature implementation plan.

## Scope and stack

- Native iOS client under `ios/`, initially targeting iOS 26+. The existing Rust hub remains authoritative for review rules, revisions, comment placement, authorization, and merging.
- Connect through the installed Tailscale app, preserving the hub's device trust model. Notifications, APNs, and background polling are out of scope.
- Support the operator's review loop: inbox, file review, conversations, persistent drafts, feedback, and approval/merge. Preserve drafts and reading position across interruptions; refresh authoritative state on foreground/reconnect.
- Swift 6 language mode, SwiftUI, Observation, and Swift concurrency; UIKit/TextKit 2 for the diff reader; URLSession/Codable for networking; SwiftData for feature-owned drafts/metadata and files for cached source.
- **Tuist** manages internal modules and generated Xcode projects. Use Swift Testing for behavior/contracts, XCTest for UI flows, and golden tests for rendering. No example apps.

## Modularization

- Feature modules: **HubConnection, Inbox, Review, Conversation, ReviewSubmission**. Separate rendering responsibilities into **DiffDocument** (source coordinates), **DiffView** (layout/interaction), and **CodeStyling** (native text attributes).
- Each feature owns its API subset, wire DTOs/mappings, event handling, caches, and persistence. No horizontal HubAPI, HubEvents, Store, or Cache modules. `crates/api` remains the wire-contract authority.
- Follow KingsChat's feature-local pattern: WebServices handle requests/mapping; Interactors own workflows/side effects; ViewModels own presentation state/validation; Builders assemble scene collaborators; Routers navigate. Add only collaborators actually needed.
- Cross-module dependencies use `<Feature>Interface`; services and DTOs stay internal unless consumed elsewhere. The app composes implementations. Keep responsibilities narrow; no catch-all Core, Utils, Shared, or DesignSystem module.
- Shared networking provides transport primitives and one multiplexed foreground SSE connection. Features interpret their own event subset and own refresh policy. Shared appearance dependencies are exposed through feature-scoped facades.

## Design skeleton

- **Inbox: Decision desk.** Group by the next human action: decisions/questions first, then reviewable items. Show the actual question/disputed comment and item identity; keep working, merging, and closed items secondary.
- **Diff: File reader.** Full-width, single-column unified diff; visible file/comparison scope and remaining work; importance-ordered files with reasons; expandable unchanged context. Change cards and before/after remain exploration alternatives, not required initial modes.
- Use native sheets for comments and review completion. Keep marking viewed separate from submission. **Approve & merge** explicitly confirms the revision and target.
- Use progressive disclosure: identity, revision/scope, required action, and progress remain visible; supporting details open on demand. Comment anchors use source coordinates, never wrapped display rows.

## Appearance and highlighting

- Match [web semantic colors and typography](ui/design-system/tokens.css): light/dark roles, **DM Sans**, weights, text styles, line heights, and tracking; system monospace for code. Support Dynamic Type and native touch sizing. Extract shared token definitions and generate web/native values to prevent drift.
- Reuse hub-computed UTF-16 syntax spans and the [web syntax palette](ui/diff-view/highlight.ts); no bundled parser initially. Apply spans to source text excluding diff prefixes/gutters, before wrapping. Show plain code until highlighting arrives; retain cached source/spans for offline reading.
