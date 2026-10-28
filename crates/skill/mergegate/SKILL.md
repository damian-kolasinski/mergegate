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
- A review's summary, its body, has no ID and takes no reply: your next `mg submit` answers it, so make the changes it asks for in that revision. Anything in it you won't do or have to ask about, put to the operator in your session.
- Weigh each comment before acting on it. Reject one that would make the code worse, and say why.
- When a command exits 2 with `needs_decision`, put its question and options to the operator and run the command again with the flag they choose. Don't choose for them.
- After `item.merged`, clean up the way you agreed with the operator (below).

## The loop

1. Commit on a branch of your own, in a clone where `mg init` ran ([references/setup.md](references/setup.md)). Start it from the target as the hub has it, `git fetch mergegate` then `git switch -c <branch> mergegate/<target>`: nothing keeps the clone's own `<target>` branch current.
2. `mg submit --title "Short summary" --guide guide.md`, the guide saying what the change is for and which files to read first ([references/authoring.md](references/authoring.md)). It prints the item's name, as in `app#3`: on GitHub that's its pull request's number, as in `app#84`, and every command takes it. With `--json`, pass its `name` back, never its `id`, which can be another item's name. Running it again from the same branch adds a revision.
3. Wait for the item's events as [references/waiting.md](references/waiting.md) describes: in a background command whose completion resumes you, `mg wait app#3`, then run it again on exit 75; in the foreground, `mg wait app#3 --timeout N` in a loop, N just under your tool-call time limit. Elsewhere the default 540 seconds keeps a wait below a host's ten-minute cap. In Claude Code, the background `mg wait app#3` waits without a timeout and only says that events came; read them with `mg feedback app#3`. In Codex, leave a wake and end your turn as the reference describes.
4. Act on them (table below), then go back to 2 or 3.

| Event | What to do |
|---|---|
| `review.submitted` | Change what the summary and the comments ask, `mg reply` to each comment, then `mg submit`. A verdict of `approved` needs nothing; wait for the merge. A review with no new comments, only replies to yours, needs nothing either unless its summary or a reply asks for something; keep waiting. |
| `external.comments` | Reviewers commented on the item's GitHub pull request. Treat them like the operator's comments: change, `mg reply` to each one, `mg submit`. See [references/authoring.md](references/authoring.md), "Comments from GitHub". |
| `comment.override` | The operator overrides your rejection of a comment: make the change, `mg reply` with `addressed`, `mg submit`. |
| `merge.conflict` | The target moved under the item, before or after review. Run the commands it lists, resolve the conflicts, `mg submit`. |
| `merge.held` | The operator retries or cancels the merge; keep waiting. If the reason is that the branch is behind its target, rebase onto it and `mg submit`: a revision whose diff is unchanged merges on the same approval. |
| `merge.cancelled` | The item needs review again; keep waiting. |
| `parent.updated` | The item this one is stacked on has a new revision, or merged without this one. Run the commands it lists, which move only your own commits, resolve any conflicts, `mg submit`. |
| `parent.abandoned` | The item this one is stacked on was abandoned. Run the commands it lists to move your own commits onto the target, or ask the operator first if your work needs the abandoned one, then `mg submit`. |
| `item.merged` | The work landed. Clean up as agreed, then stop. |
| `item.abandoned` | Stop work on the item. Ask the operator before deleting its branch. |

`mg wait` prints each event headed by its cursor and type, as in `[7] review.submitted app#3 r2: the operator approved`, and marks them delivered; in Claude Code it only names them and leaves them for `mg feedback`. It exits 0 with events, even when they closed the item, 75 when it timed out with none (wait again), and 3 when the item is merged or abandoned and nothing new is left. Stopped by a signal, it exits 143, 130 or 129 after a line that names the command resuming it; see [references/waiting.md](references/waiting.md), "When a wait ends early". `mg feedback app#3` does the same without waiting, and `--since CURSOR` reads the events after any cursor, delivered ones too.

[references/authoring.md](references/authoring.md) covers titles, revisions, stacking work too big for one review, replies and abandoning in more detail. When the operator asks you to show or hide some files in review, to rank their queue, to watch the hub for events, or to record how long their reviews take, see [references/operator.md](references/operator.md).

## After a merge

The operator merges an item by squashing it onto the target, so git doesn't see the item's branch as merged and `git branch -d` refuses it. `item.merged` names the branch and lists the default cleanup. Do what you and the operator agreed; if you never agreed anything, the default leaves the clone on the updated target, which has the squash commit, and drops the remote's branches the hub deleted after their merge:

```
git fetch --prune mergegate
git switch --detach mergegate/<target>
git branch -D <branch>
```

It leaves the clone's own `<target>` branch where it was, as it may hold the operator's work or be checked out in another worktree. Start the next branch from `mergegate/<target>` (step 1 of the loop), not from `<target>`.

## Picking up an item again

A session that ended mid-review loses nothing: `mg resume app#3` prints what a new session needs (the item, its branch and worktree, the last review, every comment still open and any notice still asking for a revision, such as a merge conflict) and marks nothing delivered, so the next `mg wait` still returns the events the old session missed. A wait the old session left running takes them instead, even after that session is gone: end it first, as [references/waiting.md](references/waiting.md) describes under "When a wait ends early".

## Reporting product failures

Use `mg diagnostics logs --since 30m --json` for operational evidence;
`--details` also includes sanitized warning/error text. `mg diagnostics collect --out diagnostics.zip` saves a private local archive, including when the local
hub cannot start. A connected machine needs the hub owner's access.

When the user asks to report a mergegate bug, write a summary and select relevant
log excerpts into UTF-8 files. `mg report prepare --title "Problem" --body summary.txt --logs selected-logs.txt --out issue-report` creates a sanitized draft
and ZIP without publishing. Review the generated report and ZIP for sensitive
content. `mg report send --from issue-report` explicitly creates a public issue
in the mergegate repository using this machine's gh login. Send only with the
user's authorization to publish the report. Return the issue URL and ZIP path;
the author can attach the ZIP manually. Known credentials and private paths are
redacted, but prose redaction is heuristic and needs inspection. A failed send
preserves the draft. Check GitHub before `--retry`; it reconciles by report ID
without creating another issue if the list has not caught up. See each command's `--help` for limits and options.
