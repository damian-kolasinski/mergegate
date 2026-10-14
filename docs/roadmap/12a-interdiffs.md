# 12a — Interdiffs

Roadmap row 12a · decisions settled 2026-10-14

## Scope

A new revision is reviewed by what changed since the operator last looked, not from scratch. Each revision is diffed against every earlier one, over a moved base, when it is submitted. Viewed marks follow files that haven't changed since, a banner says a newer revision arrived mid-review, and the inbox counts what changed since the last review. That covers F7's re-review (steps 7–9) and F13 up to submitting on the old revision; moving drafts onto the new revision is 12b, and the guard that keeps a re-changed commented file in `review` is 11b.

## Decisions

1. **Virtual rebase.** The interdiff from rK to rN replays rK onto rN's base (`git merge-tree --write-tree --merge-base=<rK base> <rN base> <rK head>`) and diffs that tree against rN's head, so the target's own changes drop out (tech-stack.md "Interdiff after the base moved"). With an unmoved base it is rK's head against rN's. A file the replay conflicts on can't be told apart from the target's changes: it shows its whole own diff in rN, marked as such.
2. **Every earlier revision, at submit.** Submitting rN computes and caches the interdiff from each of r1…rN-1, so opening any comparison stays a database read.
3. **The selector** offers *All changes* (against the merge-base) and *Since rK* for each earlier revision. It defaults to since the last revision the operator reviewed, or all changes when that is rN or there's none. A closed item shows all changes only.
4. **Viewed marks move forward while unchanged.** Marking or unmarking a file in rK applies to every later revision where the file is unchanged since rK, at the mark and when a later revision is submitted. A file the target alone changed stays viewed.
5. **The banner keeps you where you are.** A newer revision mid-review shows "rN submitted · Show changes since rK". Until you switch, comments and Request changes go to the revision on screen, and Approve is disabled. Switching opens the interdiff from the revision you were on; drafts stay on it, in the Conversation list, until 12b moves them.
6. **The inbox row** shows "N changed since rK" once the operator has reviewed an earlier revision.
7. **One PR,** per the roadmap's size rule: interdiffs, viewed marks and the screens that show them are one change.
8. **The demo** is committed to an orphan `demos` branch under `pr-<n>/` and embedded in a PR comment.
9. *(agent)* **Since mode lists every rN file:** changed, unchanged (not shown in the diff, counted under the file list), whole own diff where the replay conflicted, and *dropped* for a file rK changed that rN no longer does. Viewed counts and Approve's unviewed count cover rN's files, as before.
10. *(agent)* **Comments in since mode go on rN's lines only.** The old side is the replayed rK, which no revision has, so its lines take no comments and show no threads.
11. *(agent)* **A replayed tree is pinned** at `refs/mergegate/items/<n>/r<K>-r<N>`, so its blobs survive until the item closes and drops its pins. Interdiff diffs share the blob-pair cache and are pruned with it.
12. *(agent)* **An item submitted before this step** has no interdiffs; since mode falls back to all changes, and its marks don't move.

## Where it lives

- `core`: `interdiff` (computing at submit, reading a comparison, the inbox count; the `InterdiffGit` and `InterdiffStore` ports), `item::item_diff` takes a comparison, `item::submit` records interdiffs and carried marks with the revision, `viewed::mark` moves marks forward. `ReviewStore` gains the last reviewed revision.
- `git`: `interdiff.rs` (merge-tree replay and diff).
- `store-sqlite`: `0002_interdiffs.sql` (`interdiffs`, `interdiff_files`), `interdiffs.rs`; pruning covers interdiff pairs.
- `api::diff` and `api::item`; `hub::diff` reads `?since=`, `hub::items` fills the inbox count.
- UI: the selector and banner in `ui/item/`, since mode in the diff and file list, the inbox count.

## Stack

- `interdiffs`: everything above; F7 and F13 scenario and Playwright tests, doc updates, deleting this brief and roadmap row 12a. *In progress.*

## Open

- **12b** re-maps drafts and sent comments onto newer revisions; until then a draft on rK shows only in the Conversation list once you switch to rN.
- **11b** reads `interdiff_files` to keep a commented file in `review` when it changed again.
