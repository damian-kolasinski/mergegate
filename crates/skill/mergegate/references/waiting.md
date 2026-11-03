# Waiting for feedback

The automatic adapters wait outside the model and wake only for actionable feedback or failures. Approval-only reviews and successful merges stay silent; their events remain available through `mg feedback`. Approvals with operator-written summaries, comments or replies still wake, as do conflicts, held/cancelled merges, parent updates, overrides, external comments and operator abandonment. A successful merge supersedes older pending wakes. Where `mg skill setup` added its session hook, a session in a mergegate clone starts by being told which way fits its agent. Pick the cheapest way to wait that your environment supports:

| If you can… | Do this | Tokens while waiting |
|---|---|---|
| run Claude Code with the installed Stop hook (see below) | `mg submit --wake ""`, then end your turn | none |
| be resumed by a shell command, as Codex is (see Codex below) | `mg submit --wake "<command that resumes you>"`, then end your turn | none |
| neither | loop `mg wait app#3 --timeout N`, N just under your tool-call time limit | one short turn per timeout |

`mg wait` remains available for explicit CLI use and other harnesses; it intentionally returns informational events too. It returns once the item has events you haven't read, prints them and marks them delivered. It exits 75 when the timeout passes with none; run it again. Its default timeout is 540 seconds, below a host's ten-minute cap; choose a shorter `--timeout N` if your host's cap is lower. `--timeout 0` waits indefinitely, so use it only where the host keeps tracking commands that long. In Claude Code (`CLAUDECODE=1`) it waits without a timeout unless you give one, and when events come it names them and marks nothing delivered: `mg feedback app#3` prints and delivers them (see Claude Code below). `--for operator` ends the wait only on an event from the operator, still returning everything new with it; without it, comments from GitHub end the wait too.

A merge or an abandon ends the wait too. Events still exit 0, even when one of them closed the item, so read them all: an `approved` review and the `item.merged` with its cleanup can come in one wait. Only once nothing new is left does a wait on a closed item exit 3, printing that it is merged or abandoned. `mg feedback` exits the same way.

## When a wait ends early

Hosts end commands that run too long: a tool call's timeout, a cap on background commands, or a session that stopped tracking one. An exit such as `-1` with no output is a host result, not an `mg` exit code: recover as below instead of treating it as a quiet wait. Stopped by SIGTERM, SIGINT or SIGHUP, `mg wait` prints one line and exits 143, 130 or 129:

```text
mg wait: SIGTERM stopped the wait on app#3 before it had events; nothing was marked delivered, so resume with: mg wait app#3
```

Nothing is lost, since a wait marks events delivered only once it has printed them: run the command the line names, in the background again if that's how you wait. With `--json` the line comes as an object with `stopped`, `item`, `cursor` and `resume`. A wait that ends without that line, as one killed with SIGKILL (exit 137) or whose output the host dropped, may have printed nothing, or printed events where you can't read them and marked them delivered: run `mg feedback app#3`, which prints what's new without waiting. When it says an earlier read took the item's events (with `--json`, no events and a `cursor` above 0) and you never saw them, run the `mg resume app#3` it names and act on what it lists as still waiting on you: the comments you haven't replied to, and the notice still asking for a new revision, such as a merge conflict to rebase. Replied comments and notices a revision or a later notice answered aren't listed, so nothing gets done twice; `mg feedback app#3 --since 0` replays every event. Then wait as before.

A hub that restarts, as on an upgrade, doesn't end a wait, even one started while the hub is down: it prints why on stderr, as in `reconnecting in 1s`, and connects again, until its timeout or, without one, for two minutes. If the hub is still down by then, the wait exits 69 with a hint naming the command that resumes it once the hub is back. A refused device or incompatible API still ends it immediately with the fix and resume command.

A wait you no longer watch, left running by a session that ended or a host that lost track of it, takes the item's next events: it prints them where nobody reads and marks them delivered, so your next wait never sees them. In Claude Code a wait delivers nothing and ends within seconds of the session that started it, so none is left to take events. End it before you wait again or rely on a wake: `pkill -f "mg wait app#3( |$)"`.

## Cursors

Each event has a cursor, and the line after the events (from `mg feedback` in Claude Code, where a wait prints none) names the newest one and the command that reads on after it: `cursor 12 (mg wait app#3 --since 12 continues after it)` from `mg wait`, keeping `--for operator` when the wait had it, and `mg feedback app#3 --since 12` from `mg feedback`. When the events closed the item nothing comes after them, so the line is only `cursor 12`, and a last line says so: `app#3 is merged`. Cursors count every item's events on the hub, so one item's cursors skip numbers: app#3's may be 4, 9 and 12. `--since N` returns the item's events after cursor N, never N itself: `mg feedback app#3 --since 0` replays them all, and `--since 12` only what comes after 12. A read with `--since` marks what it printed delivered, unless it started past an event you haven't read: then nothing is marked, so the next plain `mg wait` still returns that event.

The events are what you have to act on: the operator's reviews and overrides, comments from GitHub, what happened to the merge, and the item closing, by your own `mg abandon` too. Your submits and replies are not events and get no cursor, so a replay shows how the review went from the operator's side, without your half; `mg resume app#3` shows the comments still waiting on you with your replies to them.

## Being woken

`mg submit --wake CMD` leaves a command with the hub. When the item has actionable events from the operator or from GitHub you haven't read, it runs `sh -c CMD` in the worktree you submitted from, with `{item}` in CMD replaced by the item's name and these set:

- `MG_ITEM`: the item's name, as in `app#3`.
- `MG_CURSOR`: the cursor of the newest actionable event.
- `MERGEGATE_HOME`: the data directory of the `mg` that runs it, so `mg` in the command finds the hub.

The command should start a session that runs `mg wait {item}` or `mg feedback {item}` and works on what it returns; `mg resume {item}` prints a prompt that brings a fresh session up to date. The hub runs it at most once per event, never while an `mg wait` on the item is running, never for events a wait already returned, and one at a time per item. Your own events, like your `mg abandon`, never wake you. The hub doesn't retry a failed command; the command's stderr is in `$MERGEGATE_HOME/wakes/<item>.log`.

On the hub's machine the hub runs the command. On a machine `mg connect` points at another hub, that machine's `mg wakes run` does: the hub hands it the commands left from there, and it runs them as the hub would. `mg connect` installs it as a service, so it already runs; when `mg submit --wake` warns that no `mg wakes run` polls from the machine, the wake waits until one does, so ask the operator to run `mg connect HUB` there again, and wait with `mg wait` meanwhile. A submit from another machine than the last one removes the wake the last one left, since it would start a session in a worktree the item's agent left.

The latest submit that passes `--wake` sets it; `--wake ""` removes it. Read what's new with `mg feedback` before you set it from a live session, or the hub wakes you for it at once. When you pick up an item with `mg resume`, it shows the wake an earlier session left: replace it with yours or remove it, or it keeps starting that session.

## Claude Code

Use a Claude Code version supporting command hooks with `asyncRewake` (verified with 2.1.316). `mg skill setup --agent claude` installs the Stop hook, and hub upgrades refresh an existing installation. If `mg skill status` shows Claude Code's hook missing or outdated, ask the operator to run `mg skill setup`, and until then wait with `mg wait app#3` in the background. The hook stays off in `claude -p` runs, where Claude Code would run it in line and hold the run open; a print-mode session loops `mg wait app#3 --timeout N` instead. After submitting, end your turn:

```
mg submit --wake ""
```

The empty wake clears any old headless or Codex command. The hook runs `mergegate skill claude-watch` outside the model, watches the open items submitted from this worktree on this device, and exits 2 with a short `mg feedback ITEM` instruction only for actionable events. It rearms when the next turn ends. An approval-only event keeps it waiting; once all watched items close successfully it exits 0 silently. Use one active Claude session per worktree. Items with an explicit `--wake` command remain that command's responsibility.

Do not launch a background `mg wait` alongside the hook: its completion still wakes Claude for informational events. Stop an old wait when migrating. The hook does not acknowledge feedback, and remembers which cursors it has already notified this session about. Read `mg feedback ITEM` on a wake; use `mg resume ITEM` when picking up work again. Separate submits in the same worktree are watched together, including stacked items that need a rebase after their parent merges.

The hook belongs to the live session; it does not resurrect a closed/deleted session. A hub restart, as on an upgrade, passes silently; a hub still unreachable after two minutes, or any other watcher failure, wakes once with a diagnostic, then retries after that turn ends without repeated model notifications during the same outage. After seven continuous days it asks for a turn to rearm before the host's timeout. Successful merges no longer trigger cleanup, a completion message or agent-side post-merge actions; the operator can reopen the session for those. Merge history, cleanup commands and hub review-time statistics remain available.

`mg wait` still supports explicit use in Claude: it reports pending events without acknowledging them, and stops when `CLAUDE_PID` disappears. It is a compatibility path, not the automatic adapter.

## Codex

Never wait with `mg wait` in Codex, not even in the background: Codex checks on a running command with a model turn, so every check costs one. In a Codex session (`CODEX_THREAD_ID` set) `mg wait` doesn't wait: it returns what's new or exits 75 at once. Leave a wake that queues a message for your thread instead, then end your turn. `mg wakes codex` does it; from the worktree:

```
mg submit --wake "mg wakes codex $CODEX_THREAD_ID"
```

The shell expands `$CODEX_THREAD_ID`, your thread's ID, as you submit. When the item has events, `mg wakes codex` queues a message telling the thread to run `mg feedback {item}`. Where the Codex app runs, it also loads the thread in the app's background service, which only runs the threads a window shows, so the message starts a turn at once; elsewhere the `codex` session that has the thread open runs it, and a thread none has open gets it when next opened. The wake stays set: after acting on the events, end your turn again. It runs with `python3`, and finds `codex` on `PATH`, in `~/.local/bin` or in the Codex app; elsewhere, put `CODEX_BIN=/path/to/codex` before `mg` in the command.
