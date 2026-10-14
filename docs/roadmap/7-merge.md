# 7 — Approve and merge (local)

Roadmap row 7 · decisions settled 2026-10-14

## Scope

An approved item lands on the target branch of the operator's repo, and an unwanted one closes. That covers F8 on the `local` backend (approve, merge, hold on a dirty checkout, retry), F9's conflict found at approve time, and F14's abandon. Closing an item prunes its diffs (design.md §5). Re-testing open items whenever the target moves is PR 14's early conflict detection; the GitHub merge is PR 9.

## Decisions

1. **Squash.** A merge is one commit on the target: the tree from `git merge-tree --write-tree` of the target and the item's head, with the target as its only parent. Its subject is `<title> (app#42)`.
2. **The operator authors it,** as author and committer, read from the operator repo's `user.name` and `user.email`. Nothing in it names an agent.
3. **The push leaves the operator's config alone.** Staging pushes to the upstream with `receive.denyCurrentBranch=updateInstead` passed through `--receive-pack`, never forced. A non-fast-forward (the operator committed to the target meanwhile) re-fetches, re-merges and pushes once more; a conflict there becomes `merge.conflict`.
4. **`a` opens a confirm step,** where Enter merges. It shows how many files are unviewed but doesn't block on them.
5. **Approving sends the drafts** as a review with verdict `approved`, then merges.
6. **A dirty checkout holds the item in Merging,** with git's reason and Retry; nothing retries on its own. The operator can cancel a held merge, which puts the item back in Needs review. A new revision cancels it too, as any revision does (design.md §5).
7. **Test-merge only at approve time.** Re-testing every open item when the target moves is PR 14.
8. **`merge.conflict`** carries the conflicted paths, the target commit and the commands to run (`git fetch mergegate && git rebase mergegate/<target>`). The item goes to Changes requested with a conflict badge.
9. **Abandon from `mg abandon` or the item header,** behind a confirm and not on a key. `--reason` stays optional.
10. **The agent learns of every operator action on its item,** each in its log with `source: operator`: the approval (`review.submitted`, verdict `approved`), a hold (`merge.held`, so it doesn't push over an approval), a cancelled hold (`merge.cancelled`), `item.merged` (with the squash commit) and `item.abandoned` (with who and why). `mg wait` still exits 3 on a closed item and prints the closing event.
11. *(agent)* **A new migration** rebuilds `events` for the new types, so databases already past 0006 pick it up.
12. *(agent)* **Approve merges inside its request,** under the store lock, and answers merged, held or conflict. Merges are serial as a result.
13. *(agent)* **A merge cut short by a restart leaves the item in Merging,** where Retry merges it again. If the push had landed, the merged tree equals the target's and the item closes as merged at the target commit; nothing runs at startup.
14. *(agent)* **Pruning follows design.md §5:** drop the pins, delete the cached diffs no open revision uses, re-diff a merged item on read without caching, show an abandoned item's files and counts with a note. No explicit `git gc`.
15. *(agent)* **A `Backend` port** with the local adapter only, so PR 9 adds GitHub behind it.

## Where it lives

- `core`: `item::close` (closing and pruning), `item::abandon`, `merge` (approve, the `Backend` port, hold and retry). `Verdict` gains `Approved`; `EventKind` gains the close and merge events.
- `git`: `merge.rs` (merge-tree, commit-tree, push upstream), unpinning in `items.rs`.
- `store-sqlite`: `0009_close_events.sql`, plus deleting unused diff cache rows.
- `api::merge`; `hub::abandon`, `hub::merge`; `cli::abandon_cmd`.
- UI: Approve & merge and its confirm in `ui/item/review-bar.tsx`, Abandon, the hold and Retry in `ui/item/header.tsx`.

## Stack

- `merge-1-abandon`: closing an item: `mg abandon`, Abandon in the UI, `item.abandoned`, pruning, the abandoned item's view, `mg wait` printing the closing event, this brief. *Planned.*
- `merge-2-approve`: approve and squash-merge, conflict, hold, cancel and Retry, Approve & merge in the UI, the merged item's view; F8, F9, F14 end to end, doc updates, deleting this brief and roadmap row 7. *Planned.*

## Open

- **Doc drift:** design.md §4 and §8 say "merge" where it's now a squash, and §6.3 gains `merge.held` and `merge.cancelled`; §9.2's review bar gets Approve & merge back.
- **PR 8's skill** must tell agents to delete a merged branch with `git branch -D`: after a squash git doesn't see it as merged.
- **PR 9** should squash through the GitHub API too, so both backends land work the same way.
