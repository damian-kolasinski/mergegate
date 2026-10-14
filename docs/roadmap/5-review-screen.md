# 5 — Review screen

Roadmap row 5 · decisions settled 2026-10-13

## Scope

Opening an item shows its diff: every changed file in one scrolling column with a file list beside it, driven from the keyboard. This PR delivers the viewing part of F6 ("Walk the Review tier in order", "v to mark viewed"), on the newest revision against its merge-base. The revision selector and interdiffs come in PR 12, comments in PR 6, tiers in PR 11, highlighting in PR 15, and the intent and demo panels in PRs 11 and 16 (design.md §9.2).

## Decisions

1. **The diff is one continuous scroll, like GitHub's**, rather than the one-file pane in the design.md §9.2 mock. The file list stays beside it and follows the scroll.
2. **It renders with `@pierre/diffs`' `CodeView`** (Apache-2.0), wrapped in `ui/diff-view/`. CodeView handles variable-height virtualization, scroll anchoring, sticky file headers, line selection, expandable context, and the annotation slots PR 6's comments will use. It is fed hunks the hub computed, so the lines it shows are the ones the hub anchors comments to. It renders plain text until PR 15. Cmd+F can't find lines that are scrolled out of view.
3. **Scrolling is measured, not assumed.** A Playwright benchmark scrolls a large synthetic item and reports frame times against tech-stack §9 (60 fps on a 5k-line diff). It lands with the first PR that renders.
4. **Keys follow design.md:** `n/p` file, `]/[` hunk, `j/k` move a line cursor (PR 6's `c` comments on it), `v` toggle viewed, `Escape` back to the inbox. F6's "j/k to move" is reworded to match.
5. **Viewed marks ship here**, stored per item, revision and file. The file list shows them along with a viewed count. PR 12 carries marks over to new revisions.
6. **Gaps between hunks expand.** The default is 3 lines of context. Expanding reads the file's full text through a blob endpoint, only when it's needed.
7. *(agent)* **Diffs are computed at submit and cached by blob pair** (tech-stack §1, §5): gix reads the blobs, and imara-diff (histogram) computes the hunks and the line counts. The inbox's `+/−` counts therefore match what the view shows. Git still detects renames (`-M`).
8. *(agent)* **Opening an item is one read**, `GET /api/items/{id}`, returning the item, its files and their hunks. File texts come from `GET /api/projects/{name}/blobs/{oid}`, immutable and cacheable.
9. *(agent)* **Binary files and files over 20k changed lines** show a one-line placeholder instead of hunks.

## Where it lives

- `core::diff`: hunk types, building hunks from two blobs (pure, golden-tested against `git diff --histogram`), and the `DiffGit` (read blobs) and `DiffStore` (the cache) ports. `core` gains `gix-imara-diff`, which does no I/O, in `CORE_ALLOWED`.
- `core::item`: `changed_files` carries each side's blob ID; submit diffs and caches the pairs it hasn't seen, and `item_diff` only reads.
- `git`: `diff --raw` for the file list, blob reads through gix.
- `store-sqlite`: migration `0004_diffs.sql` adds blob IDs to `revision_files` and the `diffs` cache, its hunks kept as unified-diff text. Existing items are dropped rather than backfilled, which is allowed before PR 9. Entries are never evicted: a pair's diff never changes, and the cache is far smaller than the git objects it describes. `viewed` comes in `0005`.
- `api::diff`: wire types. `hub::diff` and `hub::viewed`: the routes.
- UI: `ui/diff-view/` wraps CodeView, `ui/item/` is the review screen (header, file list, keys), and `ui/tests/review.spec.ts` plus `ui/tests/scroll.bench.ts`.

## Stack

- `review-1-diff`: the diff engine, plus this brief. *Done.*
- `review-2-cache`: blob IDs from git, diffs computed and cached at submit, migration `0004`. *Done.*
- `review-3-api`: `GET /api/items/{id}` and the blob route. *Done.*
- `review-4-screen`: CodeView wrapper, review screen, file list, keys, expansion, Playwright spec and scroll benchmark. *Planned.*
- `review-5-viewed`: viewed marks (migration `0005`, routes, `v`, checkboxes), F6 wording, deleting this brief and roadmap row 5. *Planned.*

## Open

- **Doc drift:** design.md §9.2's mock and tech-stack §4 (`diff-view/` "consumes pre-tokenized lines only") change with decisions 1 and 2. user-flows F6 changes with decision 4.
- **PR 15 decides highlighting** with CodeView in place. Its options are Shiki in a worker, as Pierre does it, which reverses tech-stack decision 3 and the language packs, or feeding the hub's tree-sitter spans through CodeView's token hooks.
