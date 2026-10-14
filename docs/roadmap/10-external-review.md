# 10 — External review

Roadmap row 10 · decisions settled 2026-10-14

## Scope

Comments that bots and teammates leave on a github item's pull request flow into mergegate (F5): the hub polls them, shows them on the review screen as they arrive, and hands them to the item's agent in batches as `external.comments`. The agent answers them with `mg reply` like the operator's, and the operator can agree with a rejection or override it (`comment.override`). `mg sync` and a Sync GitHub button refetch on demand. Re-mapping comments onto newer revisions is PR 12; the inbox's per-reviewer summary waits for later.

## Decisions

1. **Three kinds of comment:** inline review comments, conversation comments, and review bodies that aren't empty. The operator's own (the account `gh` signs in as) are skipped; GitHub's `user.type` says bot or human.
2. **An edit** changes the shown body, comes again in the next batch flagged `edited`, and restarts the quiet window. Deleted comments are ignored.
3. **The hub waits, not the agent.** Comments show in the UI when polled. An item's batch is logged once it has been quiet for 60s and either every expected bot has posted since the newest revision was pushed or 20 minutes have passed since that push; 20 minutes after its first held comment it goes regardless. A read that returns other events releases the held comments first. `external.comments` ends a plain `mg wait`, not `--for operator`, and runs wake commands.
4. **Expected bots** are a project setting: `mg project set <p> bots codex,claude`. With none, only the quiet window applies.
5. **Agree and Override** apply to an external comment whose agent rejected it. Override sends `comment.override` at once with an optional note and reopens the thread; Agree records the operator's assent to that rejection and sends nothing. Nothing goes to GitHub.
6. **`mg sync <item>` and Sync GitHub** refetch everything, ignoring cached ETags, and release the held batch at once: the self-fix when polling misses something.
7. *(agent)* **Polling** runs every 30s per open github item, with conditional GETs (an ETag per single-page list), off the store lock. `MG_GITHUB_POLL_MS`, `MG_EXTERNAL_QUIET_MS` and `MG_EXTERNAL_CAP_MS` shorten it in tests.
8. *(agent)* **External comments share `comments`** and its `c_N` IDs, so `mg reply` works unchanged. An inline comment anchors on the revision whose head is its original commit, quoting the blob; anything unplaceable, and every conversation comment, sits on the item as a whole. Each reply in a GitHub thread is its own comment.
9. *(agent)* **A batch snapshots its bodies** (`event_comments`), so an event reads the same after an edit.

## Where it lives

- `core`: `external` (ingest, anchoring, when a batch is due, release, agree, override, the store port); `Comment` gains an optional anchor and an author; `Source::External`; the `Github` port gains `viewer` and `pull_comments`.
- `store-sqlite`: `0002_external_review.sql`, `external.rs`.
- `backend-github`: `comments.rs`, ETags and pagination in `api.rs`; testkit's fake GitHub serves comments.
- `hub`: `external.rs` (the poller), `sync`, `agree` and `override` routes; `cli::sync_cmd`; the skill.
- UI: author and bot badges, item-level comments in the Conversation list, Agree / Override, Sync GitHub.

## Stack

- `external-review`: everything above, docs, deleting this brief and row 10. *In progress.*

## Open

- **Deferred:** the inbox's "codex: 2 → 2 fixed" summary, reading a bot's progress from its reactions, re-mapping outdated comments (PR 12).
