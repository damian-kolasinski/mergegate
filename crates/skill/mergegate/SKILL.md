---
name: mergegate
description: Submit work for review through the operator's mergegate hub and act on their feedback, instead of pushing branches or opening pull requests. Use when work is ready for review, while waiting on or answering review comments, when an item is merged or abandoned, and when asked to set up mergegate.
---

# mergegate

mergegate is the operator's local code review hub. You submit a branch with `mg submit`, the operator reviews it in the hub's UI, and their review comes back to you as events. Every `mg` command takes `--json` and never prompts.

## Rules

- Never `git push` and never `gh pr create`. `mg submit` pushes the branch to the hub and puts it up for review.
- Reach the hub only through `mg`. Never open its UI or call its HTTP API, not even to read: the UI is the operator's, and the hub can't tell your requests from theirs, so one could approve your own work.
- Wait for feedback the cheapest way your environment allows: see [references/waiting.md](references/waiting.md).
- Answer every comment with `mg reply`: `addressed` once your next revision has the change, `rejected` with a `--body` saying why, or `question` with a `--body` that asks.
- Weigh each comment before acting on it. Reject one that would make the code worse, and say why.
- When a command exits 2 with `needs_decision`, put its question and options to the operator and run the command again with the flag they choose. Don't choose for them.
- After `item.merged`, clean up the way you agreed with the operator (below).

## The loop

1. Commit on a branch of your own, in a clone where `mg init` ran ([references/setup.md](references/setup.md)). Start it from the target as the hub has it, `git fetch mergegate` then `git switch -c <branch> mergegate/<target>`: nothing keeps the clone's own `<target>` branch current.
2. `mg submit --title "Short summary" --guide guide.md`, the guide saying what the change is for and which files to read first ([references/authoring.md](references/authoring.md)). It prints the item's ID, as in `app#3`. Running it again from the same branch adds a revision.
3. Wait for the item's events, for example `mg wait app#3`.
4. Act on them (table below), then go back to 2 or 3.

| Event | What to do |
|---|---|
| `review.submitted` | Change what the comments ask, `mg reply` to each one, then `mg submit`. A verdict of `approved` needs nothing; wait for the merge. |
| `external.comments` | Reviewers commented on the item's GitHub pull request. Treat them like the operator's comments: change, `mg reply` to each one, `mg submit`. See [references/authoring.md](references/authoring.md), "Comments from GitHub". |
| `comment.override` | The operator overrides your rejection of a comment: make the change, `mg reply` with `addressed`, `mg submit`. |
| `merge.conflict` | The target moved under the item, before or after review. Run the commands it lists, resolve the conflicts, `mg submit`. |
| `merge.held` | The operator retries or cancels the merge; keep waiting. If the reason is that the branch is behind its target, rebase onto it and `mg submit`: a revision whose diff is unchanged merges on the same approval. |
| `merge.cancelled` | The item needs review again; keep waiting. |
| `item.merged` | The work landed. Clean up as agreed, then stop. |
| `item.abandoned` | Stop work on the item. Ask the operator before deleting its branch. |

`mg wait` prints each event headed by its cursor and type, as in `[7] review.submitted app#3 r2: the operator approved`, and marks them delivered. It exits 0 with events, even when they closed the item, 75 when it timed out with none (wait again), and 3 when the item is merged or abandoned and nothing new is left. `mg feedback app#3` does the same without waiting, and `--since CURSOR` replays the events after any cursor without marking anything.

[references/authoring.md](references/authoring.md) covers titles, revisions, replies and abandoning in more detail. When the operator asks you to show or hide some files in review, to rank their queue, or to watch the hub for events, see [references/operator.md](references/operator.md).

## After a merge

The operator merges an item by squashing it onto the target, so git doesn't see the item's branch as merged and `git branch -d` refuses it. `item.merged` names the branch and lists the default cleanup. Do what you and the operator agreed; if you never agreed anything, the default leaves the clone on the target with the squash commit:

```
git fetch mergegate
git switch --detach mergegate/<target>
git branch -D <branch>
```

It leaves the clone's own `<target>` branch where it was, as it may hold the operator's work or be checked out in another worktree. Start the next branch from `mergegate/<target>` (step 1 of the loop), not from `<target>`.

## Picking up an item again

A session that ended mid-review loses nothing: `mg resume app#3` prints what a new session needs (the item, its branch and worktree, the last review and every comment still open) and marks nothing delivered, so the next `mg wait` still returns the events the old session missed.
