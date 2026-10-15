# Waiting for feedback

No model is involved while mergegate waits: `mg wait` is one blocking command, and you spend tokens only when it returns. Pick the cheapest way to wait that your environment supports:

| If you can… | Do this | Tokens while waiting |
|---|---|---|
| run a command in the background and be told when it exits | run `mg wait app#3 --timeout 0` in the background | none |
| be resumed by a shell command | `mg submit --wake "<command that resumes you>"`, then end your turn | none |
| neither | loop `mg wait app#3 --timeout N`, N just under your tool-call time limit | one short turn per timeout |

`mg wait` returns once the item has events you haven't read, prints them and marks them delivered. It exits 75 when the timeout passes with none; run it again. `--for operator` ends the wait only on an event from the operator, still returning everything new with it; without it, comments from GitHub end the wait too.

## Being woken

`mg submit --wake CMD` leaves a command with the hub. When the item has events from the operator or from GitHub you haven't read, the hub runs `sh -c CMD` in the worktree you submitted from, with `{item}` in CMD replaced by the item's ID and these set:

- `MG_ITEM`: the item, as in `app#3`.
- `MG_CURSOR`: the cursor of the newest such event.
- `MERGEGATE_HOME`: the hub's data directory, so `mg` in the command finds the hub.

The command should start a session that runs `mg wait {item}` or `mg feedback {item}` and works on what it returns; `mg resume {item}` prints a prompt that brings a fresh session up to date. The hub runs it at most once per event, never while an `mg wait` on the item is running, never for events a wait already returned, and one at a time per item. Your own events, like your `mg abandon`, never wake you. The hub doesn't retry a failed command; the command's stderr is in `$MERGEGATE_HOME/wakes/<item>.log`.

The command runs on the hub's machine, so only an agent there can leave one: from a machine `mg connect` points at another hub, `mg submit --wake` is refused, and a plain submit removes the wake an earlier session on the hub's machine left. Wait with `mg wait` there instead.

The latest submit that passes `--wake` sets it; `--wake ""` removes it. Read what's new with `mg feedback` before you set it from a live session, or the hub wakes you for it at once. When you pick up an item with `mg resume`, it shows the wake an earlier session left: replace it with yours or remove it, or it keeps starting that session.
