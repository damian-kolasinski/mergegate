---
name: mergegate
description: Submit work for review through the operator's mergegate hub and act on their feedback, instead of pushing branches or opening pull requests. Use when work is ready for review, while waiting on or answering review comments, when an item is merged or abandoned, and when asked to set up mergegate.
---

# mergegate

mergegate is the operator's local code review hub. You submit a branch with `mg submit`, the operator reviews it in the hub's UI, and their review comes back to you as events. Every `mg` command takes `--json` and never prompts.

## Rules

- Never `git push` and never `gh pr create`. `mg submit` pushes the branch to the hub and puts it up for review.
- Wait for feedback the cheapest way your environment allows: see [references/waiting.md](references/waiting.md).
- Answer every comment with `mg reply`: `addressed` once your next revision has the change, `rejected` with a `--body` saying why, or `question` with a `--body` that asks.
- Weigh each comment before acting on it. Reject one that would make the code worse, and say why.
- When a command exits 2 with `needs_decision`, put its question and options to the operator and run the command again with the flag they choose. Don't choose for them.
- After `item.merged`, clean up the way you agreed with the operator (below).

## The loop

1. Commit on a branch of your own, in a clone where `mg init` ran ([references/setup.md](references/setup.md)).
2. `mg submit --title "Short summary"`. It prints the item's ID, as in `app#3`. Running it again from the same branch adds a revision.
3. Wait for the item's events, for example `mg wait app#3`.
4. Act on them (table below), then go back to 2 or 3.

| Event | What to do |
|---|---|
| `review.submitted` | Change what the comments ask, `mg reply` to each one, then `mg submit`. A verdict of `approved` needs nothing; wait for the merge. |
| `merge.conflict` | Run the commands it lists, resolve the conflicts, `mg submit`. |
| `merge.held` | The operator retries or cancels the merge; keep waiting. |
| `merge.cancelled` | The item needs review again; keep waiting. |
| `item.merged` | The work landed. Clean up as agreed, then stop. |
| `item.abandoned` | Stop work on the item. Ask the operator before deleting its branch. |

`mg wait` exits 0 with events, 75 when it timed out with none (wait again), and 3 once the item is merged or abandoned. It prints the events and marks them delivered; `mg feedback app#3` does the same without waiting, and `--since CURSOR` replays from any cursor without marking anything.

[references/authoring.md](references/authoring.md) covers titles, revisions, replies and abandoning in more detail.

## After a merge

The operator merges an item by squashing it onto the target, so git doesn't see the item's branch as merged and `git branch -d` refuses it. `item.merged` names the branch. Do what you and the operator agreed; if you never agreed anything, the default is:

```
git switch --detach
git branch -D <branch>
```

## Picking up an item again

A session that ended mid-review loses nothing: `mg resume app#3` prints what a new session needs (the item, its branch and worktree, the last review and every comment still open) and marks nothing delivered, so the next `mg wait` still returns the events the old session missed.
