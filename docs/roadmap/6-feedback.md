# 6 — Feedback loop

Roadmap row 6 · decisions settled 2026-10-14

## Scope

The operator's review reaches the authoring agent, and the agent's answers come back to the operator. That covers F6's last steps (draft comments on lines, then Comment or Request changes), F7 up to the agent's new revision, and F3's `mg wait`. Approve & merge comes in PR 7, external comments in PR 10, and re-mapping drafts and comments onto newer revisions in PR 12 (design.md §9.2).

## Decisions

1. **No Approve yet.** The review bar offers Comment and Request changes; Approve & merge arrives with PR 7.
2. **A comment anchors to a line range on one side** of a file in one revision (`path`, `side` old or new, `start_line`..`line`). A review also takes an optional summary. There are no file-level comments.
3. **Comments from older revisions** show inline only on their own revision. A Conversation list in the review header lists every comment as `path:line @rN` with its status and replies. PR 12 moves them inline onto newer revisions.
4. **Drafts outlive a new revision** and are submitted against the revision they were written on; the event carries that revision. Re-mapping them is PR 12.
5. **The hub keeps a delivered cursor per item.** `mg wait` and `mg feedback` return the events after it; the CLI prints them, then acknowledges them, so an event is delivered at least once. `--since CURSOR` replays from any point without acknowledging, so a replay never skips an event.
6. **Replies are threads.** `mg reply` appends to the comment's thread and the latest status counts (`addressed`, `rejected`, `question`; a rejection needs a body). The operator answers in the UI with a draft reply that goes out with the next review, in its `review.submitted`.
7. **Events quote the commented lines**, capped at a few lines, so the agent needn't open the file to know what the comment means.
8. **The UI's SSE stream stays a list of "changed" pings.** `item.changed {item}` joins `inbox.changed` so replies show live; the stream isn't resumable, and the UI refetches on reconnect.
9. *(agent)* **Drafts live on the hub**, so a reload or a second tab keeps them.
10. *(agent)* **Comment IDs are hub-wide** (`c_17`); cursors are integers, the event's position in the log.
11. *(agent)* **Only agent-facing events enter the log** for now: `review.submitted`. An agent's own submits and replies don't.
12. *(agent)* **`--for operator` picks what ends a wait, not what it returns:** the wait returns every undelivered event once one from the operator is among them, so acknowledging never skips one. `--for any` is the default.
13. *(agent)* **`--timeout 0` waits until an event**; the CLI repeats bounded long-polls. The hub's wait holds no lock while it sleeps.
14. *(agent)* **Comment keeps the item in Needs you and still notifies the agent**; Request changes moves it to Changes requested. A review needs a summary or a comment. Submitted comments can't be edited.
15. *(agent)* **"Addressed in rN"** is derived: the first revision submitted after the reply.
16. *(agent)* **Keys:** `c` comments on the cursor's line, `r` focuses the review bar's summary for Request changes.

## Where it lives

- `core::review`: reviews, drafts, comments and their replies, and the `ReviewStore` port. `core::feedback`: the event log, the delivered cursor, and the `FeedbackStore` port.
- `store-sqlite`: `0006_reviews.sql` (reviews, events, the cursor), `0007_comments.sql`, `0008_replies.sql`.
- `api::review`, `api::feedback`; `hub::reviews`, `hub::feedback` (long-poll), `hub::comments`.
- `cli`: `wait_cmd`, `feedback_cmd`, `reply_cmd`.
- UI: the review bar and comment threads in `ui/item/`, annotations in `ui/diff-view/`, `Button` and `TextArea` in the design system.

## Stack

Each PR is a vertical slice, from the store to the UI and the CLI.

- `feedback-1-review`: a review with a summary reaches the agent: the reviews route, the event log, `mg feedback`, this brief. *In review.*
- `feedback-2-review-bar`: the review bar on the item screen, with Comment and Request changes. *In review.*
- `feedback-3-wait`: `mg wait`, long-polling the log. *In review.*
- `feedback-4-comments`: draft line comments in the diff, sent with the review and quoted in its event. *Planned.*
- `feedback-5-replies`: `mg reply`, threads in the diff, the operator's draft replies, `item.changed`. *Planned.*
- `feedback-6-conversation`: the Conversation list, F6/F7 end to end, doc updates, deleting this brief and roadmap row 6. *Planned.*

## Open

- **Doc drift:** design.md §6.3's cursor becomes an integer and its comments gain `start_line` and `quote`; §6.1's `--for` follows decision 12; §9.2's review bar loses Approve until PR 7.
