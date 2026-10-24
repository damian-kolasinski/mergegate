# iOS Mobile

Agreed direction and delivery slices for implementing the client one feature at a time.

## Scope and stack

- Native iOS client under `ios/`, initially targeting iOS 26+. The existing Rust hub remains authoritative for review rules, revisions, comment placement, authorization, and merging.
- Connect through the installed Tailscale app, preserving the hub's device trust model. Notifications, APNs, and background polling are out of scope.
- Support the operator's review loop: inbox, file review, conversations, persistent drafts, feedback, and approval/merge. Preserve drafts and reading position across interruptions; refresh authoritative state on foreground/reconnect.
- Swift 6 language mode, SwiftUI, Observation, and Swift concurrency; UIKit/TextKit 2 for the diff reader; URLSession/Codable for networking; SwiftData for feature-owned drafts/metadata and files for cached source.
- **Tuist** manages internal modules and generated Xcode projects. Use Swift Testing for behavior/contracts, XCTest for UI flows, and golden tests for rendering. No example apps.

## Modularization

- Feature modules: **HubConnection, Inbox, Review, Conversation, ReviewSubmission**. Separate rendering responsibilities into **DiffDocument** (source coordinates), **DiffView** (layout/interaction), and **CodeStyling** (native text attributes).
- Each feature owns its API subset, wire DTOs/mappings, event handling, caches, and persistence. No horizontal HubAPI, HubEvents, Store, or Cache modules. `crates/api` remains the wire-contract authority.
- Use feature-local responsibilities: WebServices handle requests/mapping; Interactors own workflows/side effects; ViewModels own presentation state/validation; Builders assemble scene collaborators; Routers navigate. Add only collaborators actually needed.
- Cross-module dependencies use `<Feature>Interface`; services and DTOs stay internal unless consumed elsewhere. The app composes implementations. Keep responsibilities narrow; no catch-all Core, Utils, Shared, or DesignSystem module.
- Shared networking provides transport primitives and one multiplexed foreground SSE connection. Features interpret their own event subset and own refresh policy. Shared appearance dependencies are exposed through feature-scoped facades.

## Design skeleton

- **Inbox: Decision desk.** Group by the next human action: decisions/questions first, then reviewable items. Show the actual question/disputed comment and item identity; keep working, merging, and closed items secondary.
- **Diff: File reader.** Full-width, single-column unified diff; visible file/comparison scope and remaining work; importance-ordered files with reasons; expandable unchanged context. Change cards and before/after remain exploration alternatives, not required initial modes.
- Keep the file reader compact: identity and guide in navigation, title above a scope/remaining-work row, then a full-width file picker with position and path. Put file reasons next to the code; avoid duplicate identity blocks or a separate row of large navigation buttons.
- Use native sheets for comments and review completion. Keep marking viewed separate from submission. **Approve & merge** explicitly confirms the revision and target.
- Use progressive disclosure: identity, revision/scope, required action, and progress remain visible; supporting details open on demand. Comment anchors use source coordinates, never wrapped display rows.

## Appearance and highlighting

- Match [web semantic colors and typography](ui/design-system/tokens.css): light/dark roles, **DM Sans**, weights, text styles, line heights, and tracking; system monospace for code. Support Dynamic Type and native touch sizing. Extract shared token definitions and generate web/native values to prevent drift.
- Reuse hub-computed UTF-16 syntax spans and the [web syntax palette](ui/diff-view/highlight.ts); no bundled parser initially. Apply spans to source text excluding diff prefixes/gutters, before wrapping. Show plain code until highlighting arrives; retain cached source/spans for offline reading.

## Vertical delivery slices

Implement in order, building on verified slices. Each slice must work through the real app, hub, and disposable review repository without stubs or placeholder actions. Include its required infrastructure, shared styling, and relevant accessibility checks in that slice.

- **Connect and reopen:** install the Tuist-built app, connect over Tailscale, display real inbox items, and retain the chosen hub after relaunch.
- **Triage the inbox:** navigate the Decision desk across projects and item states, with real questions, disagreements, and remaining-work counts.
- **Start a review:** open an inbox item, read its guide and importance-ordered files, and display the first real unified diff.
- **Read source context:** navigate files, expand unchanged context, wrap long lines, and select/copy the correct source text.
- **Read highlighted code:** render hub spans with the shared syntax palette, including plain-text fallback and highlighting arriving later.
- **Track viewed files:** mark/unmark files and verify persisted progress after reopening the review and returning to the inbox.
- **Keep comment drafts:** create, edit, and delete comments anchored to either diff side; restore them after terminating and reopening the app.
- **Follow conversations:** open existing threads from their source lines and inspect comments, replies, and their current status.
- **Send feedback:** submit draft comments and a summary; verify the hub receives the correct revision, anchors, and text.
- **Request changes:** submit actionable feedback and observe the item move back to the working state in the inbox.
- **Re-review a revision:** submit a changed revision through the CLI, compare since the last review or against the base, and verify comment/viewed-state carryover.
- **Resolve pending decisions:** answer questions and override rejections with verified feedback delivery; accept a rejection and verify its pending inbox status clears.
- **Approve and merge:** confirm the exact revision and target, merge a disposable item, and verify the target commit and closed inbox state; reject stale approval.
- **Stay current while open:** receive new items, revisions, and conversation updates through SSE; reconnect/refetch without discarding an active draft or silently changing its revision.
- **Resume offline work:** reopen cached reviews and highlighting without the hub, preserve drafts/position, then reconnect and reconcile against the current revision before submission.
