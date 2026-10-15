# 12b — Re-mapping

Roadmap row 12b · decisions settled 2026-10-15

## Scope

Comments follow the code onto newer revisions. A thread, the operator's or one from GitHub, shows on the revision on screen at the lines its own lines became, and drafts written on an older revision move onto the revision a review is submitted on, or are flagged so the operator re-places or drops them. That covers F7's step 9 (checking each comment's status next to the new code) and F13 from the switch to the review delivered; the guard that keeps a re-changed commented file in `review` is 11b.

## Decisions

1. **Threads follow their lines.** A comment written on rK shows on the revision on screen at the lines its own lines map to, through a line diff of its file between the two revisions (base to base for the old side). Lines that changed since place it on the lines that replaced them, marked "changed since rK". One whose lines were deleted outright, or that the revision's diff doesn't show, is *outdated*: it shows only in the Conversation list.
2. **Drafts move only onto unchanged lines.** Submitting a review, or approving, on a revision newer than a draft's moves the draft there, with its new lines and quote. A draft whose lines changed or are gone is flagged, and the submit waits until each flagged one is re-placed (pick lines, move it there) or dropped (F13). Before the submit, a draft that will move already shows at its new lines, marked "from rK".
3. **Placed on read.** Opening an item, and rereading its conversation, places its comments by diffing each commented file between the two revisions in memory, reusing a cached diff for the blob pair when there is one; nothing is stored, so a comment written on an old revision or ingested late needs no extra write path. This is the one exception to "request paths only read", kept because `open_item_budget` measures it end to end against the 50 ms budget.
4. **The agent's view stays as it was:** events and `mg resume` show each comment where it was written; a moved draft is sent anchored on the review's revision.
5. **One PR,** per the roadmap's size rule; the demo goes to the orphan `demos` branch under `pr-<n>/` and into the PR body.
6. *(agent)* **Placed onto the revision on screen,** not always the newest: the conversation reread takes `?onto=`, so a thread doesn't jump while the newer-revision banner holds the old diff. A comment from a revision after it isn't placed.
7. *(agent)* **Files match by path;** a file renamed between the revisions leaves its comments outdated.
8. *(agent)* **Re-placing** is an edit of the draft's anchor (`PUT …/drafts/{c}/anchor`), validated as a new draft is, so the draft keeps its ID and body.

## Where it lives

- `core`: `remap` (`lines` maps a line range through hunks; `place` places comments onto a revision), `review::submit` and `merge::approve` move drafts with the review, `review::draft` gains `move_draft`.
- `store-sqlite`: `insert_review` applies the moves; `set_draft_anchor`. No migration.
- `api::review` (`Placement`, `MoveDraft`); `hub::diff`, `hub::conversation`, `hub::comments`.
- UI: placed threads and their marks in `ui/item/comments/`, flagged drafts and re-placing in the Conversation list and review bar.
- `cli/tests/contract.rs`: `open_item_budget`.

## Stack

- `remapping`: everything above; F7 and F13 scenario and Playwright tests, doc updates, deleting this brief and roadmap row 12b. *In progress.*

## Open

- **13** builds on this to keep a stacked child's threads across rebases.
- **user-flows F13** says the review is delivered against r2 whatever the operator chose; it goes to the revision on screen (12a decision 5).
