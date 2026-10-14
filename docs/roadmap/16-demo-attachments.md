# 16 — Demo attachments

## Scope

`mg submit --attach FILE...`, `mg attach <item> FILE...`, the demo panel on the review screen and a demo marker in the inbox ([design.md](../design.md) §6.1, §9.1, §9.2). Covers the `--attach` step of F4 and "watch the demo" in F6 ([user-flows.html](../user-flows.html)). Also `mg hub set attachment-ttl`, the first hub-wide setting (§6.0).

## Decisions

Settled with the operator:

1. **One PR.** The slice is one capability; nothing in it is worth merging alone.
2. **Media only, up to 100 MB a file:** PNG, JPEG, GIF, WebP, and MP4 (any ISO-BMFF `ftyp`, QuickTime's too) or WebM video. The type comes from the bytes, never the name; SVG, HTML and everything else are refused, since the hub serves them from its own origin.
3. **Attachments belong to a revision.** The panel shows the newest revision's; when it has none, the newest earlier revision's, labeled "from rN", so a small fix needn't re-record.
4. **Closed items keep theirs for a TTL** (`mg hub set attachment-ttl 2d`, the default), then the rows and the files no other row uses go. The hub prunes on start, hourly, after an item closes and when the TTL changes.
5. **GitHub items keep demos in mergegate.** GitHub has no upload API, and a link to a local hub is no use to teammates.
6. **This PR's own demo** lives on an orphan `demos` branch, embedded in the PR body by blob URL.

Taken by the agent:

7. **Stored by content hash** under `$MERGEGATE_HOME/attachments/`, so the same file is kept once. Migration `0004` adds `attachments` and `items.closed_at`; closed items already in the database count as closed at the upgrade.
8. **One upload route,** `POST /api/items/{id}/attachments?name=…[&revision=N]`, a raw body. `mg submit --attach` checks every file before it pushes, then uploads each to the revision the submit returned; an upload that fails after the submit names `mg attach`. `mg attach` adds to the newest revision. Attachments only accumulate.
9. **Served at `GET /api/attachments/{sha256}`** with the stored type, `nosniff`, a `sandbox` CSP and single-range requests (video seeking needs them), cached as immutable.
10. **Attaching tells the UI, not the agent:** `item.changed` and `inbox.changed`; no event in the item's log, no state change.
11. **The panel** is a strip of thumbnails under the header; a click opens a viewer (Escape closes, ←/→ step), videos with controls.

## Where it lives

- `core::attachment` (media sniffing, attach, demo choice, prune), `core::settings` (hub settings, the TTL), with `AttachmentStore` and `SettingsStore` ports.
- `store-sqlite`: `attachments.rs`, `settings.rs` (over `hub_meta`), migration `0004_attachments.sql`; `items.closed_at` set by `notify`.
- `api::attachment`, `api::settings`; `ItemDiff.demo`, `InboxItem.demo`.
- `hub`: `attachments.rs` (upload, serve, pruner), `settings.rs`.
- `cli`: `attach_cmd.rs`, `--attach` on submit, `mg hub set`.
- `skill`: `authoring.md` (demos), `setup.md` (TTL).
- `ui/item/demo/`, the inbox marker; `ui/tests/demo.spec.ts`.

## Stack

- `step-16-demo-attachments` — demo attachments end to end — in review.

## Open

- Removing or replacing a wrong attachment.
- Disk use isn't reported (`mg doctor` could show the attachments folder's size).
- With a revision selector (PR 12) the panel should follow the selected revision.
