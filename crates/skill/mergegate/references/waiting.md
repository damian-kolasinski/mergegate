# Waiting for feedback

No model is involved while mergegate waits: `mg wait` is one blocking command, and you spend tokens only when it returns. Pick the cheapest way to wait that your environment supports:

| If you can… | Do this | Tokens while waiting |
|---|---|---|
| run a command in the background and be told when it exits, as Claude Code can (see Claude Code below) | run `mg wait app#3 --timeout 0` in the background | none |
| be resumed by a shell command, as Codex is (see Codex below) | `mg submit --wake "<command that resumes you>"`, then end your turn | none |
| neither | loop `mg wait app#3 --timeout N`, N just under your tool-call time limit | one short turn per timeout |

`mg wait` returns once the item has events you haven't read, prints them and marks them delivered. It exits 75 when the timeout passes with none; run it again. `--for operator` ends the wait only on an event from the operator, still returning everything new with it; without it, comments from GitHub end the wait too.

A merge or an abandon ends the wait too. Events still exit 0, even when one of them closed the item, so read them all: an `approved` review and the `item.merged` with its cleanup can come in one wait. Only once nothing new is left does a wait on a closed item exit 3, printing that it is merged or abandoned. `mg feedback` exits the same way.

## When a wait ends early

Hosts end commands that run too long: a tool call's timeout, a cap on background commands, or a session that stopped tracking one. Stopped by SIGTERM, SIGINT or SIGHUP, `mg wait` prints one line and exits 143, 130 or 129:

```text
mg wait: SIGTERM stopped the wait on app#3 before it had events; nothing was marked delivered, so resume with: mg wait app#3
```

Nothing is lost, since a wait marks events delivered only once it has printed them: run the command the line names, in the background again if that's how you wait. With `--json` the line comes as an object with `stopped`, `item`, `cursor` and `resume`. A wait that ends without that line, as one killed with SIGKILL (exit 137) or whose output the host dropped, may have printed nothing, or printed events where you can't read them and marked them delivered: run `mg feedback app#3`, which prints what's new without waiting. When it says an earlier read took the item's events (with `--json`, no events and a `cursor` above 0) and you never saw them, run the `mg resume app#3` it names and act on the comments it lists as still waiting on you: replied ones aren't listed, so nothing gets done twice. For events that aren't comments, such as a merge conflict, `mg feedback app#3 --since 0` replays them all. Then wait as before.

A hub that restarts, as on an upgrade, doesn't end a wait: it prints why on stderr, as in `reconnecting in 1s`, and connects again, until its timeout or, without one, for two minutes. If the hub is still down by then, the wait exits 69 with a hint naming the command that resumes it once the hub is back.

A wait you no longer watch, left running by a session that ended or a host that lost track of it, takes the item's next events: it prints them where nobody reads and marks them delivered, so your next wait never sees them. It also keeps the hub from running your wake. End it before you wait again or rely on a wake: `pkill -f "mg wait app#3( |$)"`.

## Cursors

Each event has a cursor, and the line after the events names the newest one and the command that reads on after it: `cursor 12 (mg wait app#3 --since 12 continues after it)` from `mg wait`, keeping `--for operator` when the wait had it, and `mg feedback app#3 --since 12` from `mg feedback`. When the events closed the item nothing comes after them, so the line is only `cursor 12`, and a last line says so: `app#3 is merged`. Cursors count every item's events on the hub, so one item's cursors skip numbers: app#3's may be 4, 9 and 12. `--since N` returns the item's events after cursor N, never N itself: `mg feedback app#3 --since 0` replays them all, and `--since 12` only what comes after 12. A read with `--since` marks what it printed delivered, unless it started past an event you haven't read: then nothing is marked, so the next plain `mg wait` still returns that event.

The events are what you have to act on: the operator's reviews and overrides, comments from GitHub, what happened to the merge, and the item closing, by your own `mg abandon` too. Your submits and replies are not events and get no cursor, so a replay shows how the review went from the operator's side, without your half; `mg resume app#3` shows the comments still waiting on you with your replies to them.

## Being woken

`mg submit --wake CMD` leaves a command with the hub. When the item has events from the operator or from GitHub you haven't read, it runs `sh -c CMD` in the worktree you submitted from, with `{item}` in CMD replaced by the item's name and these set:

- `MG_ITEM`: the item's name, as in `app#3`.
- `MG_CURSOR`: the cursor of the newest such event.
- `MERGEGATE_HOME`: the data directory of the `mg` that runs it, so `mg` in the command finds the hub.

The command should start a session that runs `mg wait {item}` or `mg feedback {item}` and works on what it returns; `mg resume {item}` prints a prompt that brings a fresh session up to date. The hub runs it at most once per event, never while an `mg wait` on the item is running, never for events a wait already returned, and one at a time per item. Your own events, like your `mg abandon`, never wake you. The hub doesn't retry a failed command; the command's stderr is in `$MERGEGATE_HOME/wakes/<item>.log`.

On the hub's machine the hub runs the command. On a machine `mg connect` points at another hub, that machine's `mg wakes run` does: the hub hands it the commands left from there, and it runs them as the hub would. `mg connect` installs it as a service, so it already runs; when `mg submit --wake` warns that no `mg wakes run` polls from the machine, the wake waits until one does, so ask the operator to run `mg connect HUB` there again, and wait with `mg wait` meanwhile. A submit from another machine than the last one removes the wake the last one left, since it would start a session in a worktree the item's agent left.

The latest submit that passes `--wake` sets it; `--wake ""` removes it. Read what's new with `mg feedback` before you set it from a live session, or the hub wakes you for it at once. When you pick up an item with `mg resume`, it shows the wake an earlier session left: replace it with yours or remove it, or it keeps starting that session.

## Claude Code

Run `mg wait app#3 --timeout 0` with the Bash tool's `run_in_background`: Claude Code tells the session when the command exits, and the turn that follows reads its output. Claude Code ends a background command that runs past its cap, so a long wait can end with the line above (exit 143); run the command it names in the background again.

To be woken after your turn ends with no wait running, leave a wake that resumes your session headless:

```
mg submit --wake "claude -p --resume $CLAUDE_CODE_SESSION_ID 'Run mg feedback {item} and act on what it returns.'"
```

The shell expands `$CLAUDE_CODE_SESSION_ID`, your session's ID, as you submit. When the item has events, the hub runs a turn of that session in the worktree; the desktop app or `claude --resume` shows it once the session is opened again. The headless turn can't ask for permission, so a tool call that would ask fails: the commands the loop needs (`mg`, `git`, the project's build) have to be allowed in the project's settings.

## Codex

Never wait with `mg wait` in Codex, not even in the background: Codex checks on a running command with a model turn, so every check costs one. In a Codex session (`CODEX_THREAD_ID` set) `mg wait` doesn't wait: it returns what's new or exits 75 at once. Leave a wake that queues a message for your thread instead, then end your turn. This skill's `scripts/codex_wake.py` does it; from the worktree, with the skill folder's absolute path:

```
mg submit --wake "python3 /path/to/mergegate/scripts/codex_wake.py $CODEX_THREAD_ID"
```

The shell expands `$CODEX_THREAD_ID`, your thread's ID, as you submit. When the item has events, the script queues a message telling the thread to run `mg feedback {item}`. Where the Codex app runs, it also loads the thread in the app's background service, which only runs the threads a window shows, so the message starts a turn at once; elsewhere the `codex` session that has the thread open runs it, and a thread none has open gets it when next opened. The wake stays set: after acting on the events, end your turn again. The script finds `codex` on `PATH`, in `~/.local/bin` or in the Codex app; elsewhere, put `CODEX_BIN=/path/to/codex` before `python3` in the command.
