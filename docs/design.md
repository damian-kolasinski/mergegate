# mergegate — system & UX design

Status: draft v0.1 · 2026-10-13 · tech stack in [tech-stack.md](tech-stack.md)

## 1. Problem

A developer runs dozens of coding agents in parallel. Each agent finishes work, self-reviews, and opens a GitHub PR.
The developer then reviews in the GitHub web UI and copy-pastes feedback back into the agent's conversation.

Pain points:

- **Context switching** between GitHub and the agent conversation, and manual copy-paste of feedback.
- **Flat file list.** GitHub shows every changed file with equal weight. Some file kinds (localization, SwiftUI views, `.pbxproj`, tests) are reliably correct and don't need human eyes, today handled by agents marking them "viewed" via API.
- **No cross-PR prioritization.** With dozens of open PRs, nothing tells the developer which one unblocks the most future work.
- **Agents are bound to the GitHub API** for reading feedback, instead of a local, richer channel.

## 2. Goals / non-goals

Goals

- Local review UI that replaces the GitHub web UI for day-to-day review, while keeping GitHub (or plain git) as the system of record.
- Files ordered by review relevance; trusted file kinds pushed out of the way.
- Inbox across all pending work, ordered by priority.
- Stacked items: an agent splits work too big for one review into a stack, each item reviewed on its own and landed bottom first (§5.1).
- A CLI that any agent (Claude Code, Codex, opencode, Cursor, …) can use to submit work and receive feedback, so the feedback loop is closed without copy-paste.
- External reviewers keep working: cloud bots (Codex / Claude GitHub connectors) and any human who comments on the GitHub PR. Their comments flow into mergegate, the agent addresses them, and the operator sees each comment's status.
- Works across machines on a tailnet: agents on worker machines, UI on the operator's machine.
- Open source, runnable by anyone, zero hosted infrastructure, useful with zero configuration (no LLM key required).
- **Agent-operated.** The operator never has to run `mg` themselves. They ask their agent ("set up mergegate for this repo", "add worker-2"), and the agent runs the commands, asking the operator only when a decision is theirs.

Non-goals (v1)

- Multi-reviewer / team review workflows. It's a *personal* tool; each developer runs their own hub.
- Hosting, accounts, or any server we operate.
- Editing code in the UI.
- GitLab / Bitbucket backends (but the backend is an interface, see §8).
- Multi-repo inbox UX (but the data model is multi-project from day one).
- CI integration and "merge when green". CI status may be displayed if present, but nothing waits on it.
- Posting anything back to GitHub other than branches, PRs, and merges.
- Hub-wide hooks that run commands on events. Harnesses follow `mg events --follow` instead (§6.4).

## 3. Concepts

| Term | Meaning |
|---|---|
| **Operator** | The human reviewing. One per hub. |
| **Hub** | The `mergegate` daemon on the operator's machine. Owns all state, serves the UI and the API, talks to GitHub. |
| **Worker** | Any machine where agents run. Has only the `mg` CLI (same binary, client mode). The hub machine is usually also a worker. |
| **Project** | A repository registered with the hub. Every ID is project-scoped (`app#42`) so multi-repo is a UI change, not a data migration. |
| **Backend** | Per project: `github` (PRs, external comments, merge via API) or `local` (plain git; merges land on the target branch of the operator's repo on the hub machine). |
| **External reviewer** | Anyone commenting on the GitHub PR who isn't the operator: cloud bots or human teammates. |
| **Item** | A unit of work awaiting review (≈ a PR). Identified by project + branch. |
| **Revision** | One submitted snapshot of an item (`r1`, `r2`, …). Every `mg submit` on the same branch adds a revision. |
| **Stack** | Items built on each other. An item submitted with `--on <item>` is that item's *child*: its diff is against the parent, and it lands after it. |
| **Authoring agent** | The agent that produced the item. Submits it, waits for feedback, and addresses it. |
| **Operator agent** | The agent the operator talks to that plans and dispatches work. It knows what blocks what, so it sets item priority. |
| **Event** | Anything an authoring agent should react to: operator review, external comments, merge conflict, a change to its parent item, merged. |
| **Tier** | Which group a changed file is in: `review` (ordered by importance) or `trusted` (hidden). |
| **Rule** | A deterministic matcher that assigns a tier. |
| **Judge** | Optional model that ranks and annotates files (and can help rank items). Pluggable provider. |

## 4. Topology

```
 worker machine (N of them)                       operator machine
┌───────────────────────────┐                ┌───────────────────────────────────┐
│ claude / codex / opencode │                │ hub daemon                        │
│ / cursor agent            │                │  ├─ HTTP API  (CLI, UI)           │
│     │ shell               │   tailnet      │  ├─ git smart-HTTP remote         │
│     ▼                     │  HTTP + git    │  ├─ state DB, attachments store   │
│   mg CLI ─────────────────┼───────────────▶│  ├─ staging bare repo per project │
│   git worktrees           │                │  ├─ GitHub sync (poll)            │
└───────────────────────────┘                │  └─ judge (optional provider)     │
                                             │ UI in browser / app ◀── operator  │
                                             └──────────┬──────────────┬─────────┘
                                          merge (local) │              │ push, PR, merge,
                                                        ▼              ▼ read comments
                                   operator's repo on this machine   GitHub
                                   (main / master / …)               (github backend)
```

Principles

- **The hub is the only stateful component and the only thing that talks to GitHub.** Workers need no GitHub credentials.
- **Git moves the code; the hub API handles coordination.** The hub is a git remote (`mergegate`). `mg submit` pushes commits to it and then registers the revision via the API. The hub never reaches into a worker's filesystem.
- **One code path for both backends.** Code always arrives at the hub's *staging* bare repo the same way. It is the hub's private workspace, so the daemon never touches the operator's working tree while you work.
- **Where code lands depends on the backend.** The project's *upstream* is either GitHub (`github`) or the operator's existing repo on the hub machine (`local`).
  - `github`: the hub pushes the branch to GitHub under the agent's branch name and opens or updates the PR: titled as the item (and retitled with it), with an empty body, never a draft. The push leases the branch from where the hub last pushed it (staging remembers that per branch), so anything pushed there by someone else stops the submit (`branch_moved`) instead of being overwritten until the agent builds on it, while a retry after a submit that failed partway, or a later item reusing a merged item's branch name, goes through. An open item whose branch is gone from GitHub, as closing its PR there may leave it, stops at `branch_gone` rather than pushing to a PR GitHub won't reopen. An existing open PR from the branch into the item's target is adopted. A submit waits for GitHub; if GitHub can't be reached, the submit fails and nothing is recorded. Merging test-merges in staging first (a conflict is a `merge.conflict`, as on `local`), then squash-merges through the API at the approved head only, deletes the branch and re-fetches the target. A retried merge that GitHub already made is recorded as merged. A merge GitHub refuses (branch protection, required checks) is held with GitHub's reason. Abandoning closes the PR and deletes the branch, and waits for GitHub too.
  - `local`: the hub squashes the item in staging (`git merge-tree`, no working tree needed), then pushes that one commit to the operator's repo with `receive.denyCurrentBranch=updateInstead` semantics. If the target branch is checked out and clean, the checkout fast-forwards in place. If it has uncommitted changes, the merge is held and the UI says why. Nothing is ever force-updated.
- **Every item lands as one squash commit** on the target, on both backends: authored and committed by the operator (their repo's `user.name` and `user.email`), with the subject `<title> (app#42)`. Nothing in it names an agent. On `github` GitHub writes the commit, so its author is the operator's GitHub account (the one `gh` is signed in as) and its committer is GitHub. Since git doesn't see the item's branch as merged afterwards, agents delete it with `git branch -D`.
- **Workers take the target from the hub.** `git fetch mergegate` returns the latest target (mirrored from the upstream), so new worktrees start from what was actually merged. The hub re-fetches the target from the upstream before every fetch and submit, so a worker's push to it doesn't stick.
- **Every revision is pinned.** The hub keeps each revision's head at `refs/mergegate/items/<n>/r<rev>`, hidden from workers' pushes and fetches, so a force-push to the branch can't drop the commits an older revision points at.
- **Diffs are computed on the hub** from staging: the merge-base against the target (or the parent revision for a stacked item, §5.1), and interdiffs between revisions.

### Multi-machine & security

- By default the hub listens on `127.0.0.1` only. `mg hub set listen tailnet` also binds it to its Tailscale IPv4 on the same port, at once, and answers with the URL a worker connects to, `http://<MagicDNS short name>:<port>`. A hub started before Tailscale is up keeps retrying, and `mg doctor` says why it isn't listening. The Host guard on that listener accepts only the hub's short name, full MagicDNS name and tailnet addresses, so DNS rebinding stays shut.
- **Tailscale is the only way in from another machine.** There are no tokens and no listening beyond loopback and the tailnet. For each tailnet connection, the hub asks Tailscale who is calling (`whois`). Devices owned by the same Tailscale user as the hub are trusted automatically. Other devices, tagged ones included, need `mg hub allow <device>`, and `mg hub deny <device>` shuts out any device, the operator's own too. Decisions are kept by the device's stable ID, so a rename keeps them, and a short name two devices share is refused in favour of the full one. A deny or `listen local` also ends that device's open `mg wait` and UI stream, not just its next request. There's nothing to copy between machines, so an agent can connect a worker end to end (F2).
- **A device the hub refuses gets an error, not a decision.** It names the device and its owner and hints `mg hub allow <device>` on the hub, and `mg hub devices` lists the device as asked. A worker can't answer a `needs_decision` that would let itself in.
- **A trusted device reaches everything a loopback caller does,** the UI included, so the operator can review from a laptop. Whether a request is remote is decided by the listener it arrived on, never by anything the client sends.
- `mg connect <hub>` checks that the hub lets this device in, then writes `connect.json` in `MERGEGATE_HOME`, which wins over `hub.json`; nothing is written when it is refused. `mg connect --local` goes back to the hub on the machine. A connected `mg` refuses commands that hand the hub a path on its own machine (`mg project add`, `mg lang add --from`, `mg project set upstream`): the hub can't read it. `mg hub start | stop | status` always mean the local hub, and status names the connection.
- `mg wait` connections are resumable via an event cursor, so a dropped connection never loses feedback.
- **The hub signs in to GitHub as the operator's `gh` does.** It runs `gh auth token --hostname github.com` and keeps the token in memory only, never on disk or in logs, reading it again after GitHub turns it down. Git gets it as an HTTP header through the environment, scoped to the repository's URL, so it never shows in the process list or reaches another remote. A logged-out `gh` is an error whose fix is `gh auth login`. Only github.com is supported.
- Only a worker on the hub's machine may set `mg submit --wake`: the hub runs whatever command it is given. The hub decides by the connection, not by anything the client sends: a `--wake` that arrives over the tailnet, even an empty one, is refused with an error before anything is recorded, so the agent knows to wait with `mg wait` instead. Since a wake runs in the worktree of the item's latest submit, a submit from another machine clears the item's wake, so a remote worker can't choose the directory a trusted command starts in.

## 5. Item lifecycle

```
   mg submit                         mg submit (new revision)
       │                    ┌──────────────────────────────────────┐
       ▼                    │                                      │
 ┌──────────────┐  request changes   ┌───────────────────┐         │
 │ needs review │───────────────────▶│ changes requested │─────────┘
 └──────┬───────┘                    └───────────────────┘
        │ approve & merge                      ▲
        ▼                                      │ merge conflict (event to agent)
 ┌──────────────┐   conflict / upstream dirty  │
 │   merging    │──────────────────────────────┘
 └──────┬───────┘
        ▼
 ┌──────────────┐          (any state) ──abandon──▶ abandoned
 │    merged    │
 └──────────────┘
```

- **Items are numbered per project** (`app#42`), with one open item per project and branch. A new revision puts the item back in *needs review*, whatever open state it was in.
- External review runs **in parallel** with operator review, mirroring today's workflow. Items enter *needs review* immediately; external comments and the agent's responses appear as they come in.
- **Approve means merge.** There's no CI to wait for, so an approval merges right away (a stacked item right after its parent, §5.1). If the merge can't happen (conflict), the item goes back to the agent with a `merge.conflict` event that names the paths and the commands to rebase. If the upstream checkout is dirty (local backend), it stays in *merging* until the operator retries; nothing retries on its own. The operator can cancel a held merge instead, which puts the item back in *needs review*, as a new revision does.
- **The agent hears every operator action on its item:** the approval (`review.submitted` with verdict `approved`), a hold (`merge.held`, so it doesn't push over an approval), a cancelled hold (`merge.cancelled`), the merge (`item.merged`, with the squash commit) and an abandon (`item.abandoned`, with who and why).
- **A merge cut short by a restart** leaves the item in *merging*, where Retry runs it again. If the push had landed, the merged tree equals the target's and the item closes as merged at the target commit.
- A new revision while the operator is mid-review never discards draft comments (§9.2).
- **Conflicts are detected early.** The hub test-merges every open item into its target on each submit, after each of its own merges, and by fetching every project's target each minute, so a target moved outside the hub is seen too. An item that no longer merges cleanly goes back to its agent with the same `merge.conflict` and moves to *changes requested*, before the operator gets to it; the operator's draft comments stay. Each revision hears of it at most once, and items in *merging* are left to their merge. The fetches and test-merges run without holding the hub up: each target is fetched aside, to `refs/mergegate/watch/<target>`, and staging's target only fast-forwards to it, so an agent told to rebase rebases onto where the target is. Open items of one project and target are also test-merged into each other: the inbox shows which ones conflict, and no agent hears of it, since only landing one makes it real for the other.
- **Closing an item prunes its diffs.** When an item merges or is abandoned, the hub drops its revision pins (a merged item keeps its newest revision's) and its branch in staging, unless that moved past the newest revision or an open item holds it, so a branch named under it (`feat/next` after `feat`) can be pushed; the hub retries that for every closed item when it starts. It also deletes the cached file diffs no open item's revision still uses (the cache is keyed by blob pair, so items and revisions share entries). Its file list and line counts stay. Opening a merged item diffs its files again on read, without caching them, since its blobs live on in the target's history and that pin, which a squash needs when it auto-merged a file the target also changed; an abandoned item's blobs may be garbage-collected, so it shows the file list and counts with a note that the diff is gone.

### 5.1 Stacked items

An agent splits work too big for one review into a stack: `mg submit --on app#41` makes the new item a child of `app#41`. Stacks are for splitting one piece of work; independent work still starts from the target. A child always uses its parent's project, backend and target.

- **Own diff only.** A child's diff is against the parent revision it is built on: the newest revision of the parent that its branch contains. The operator reviews only what the child adds, even while the child is behind its parent.
- **The agent keeps the child current; the hub never writes to an item's branch.** The child's agent gets `parent.updated` when the parent gets a new revision, or lands without its commits reaching the target (squash or rebase merge). The event carries the exact command (`git rebase --onto <new base> <old parent revision>`), and the agent submits the result. Until then the inbox shows the child as behind its parent.
- **Viewed marks and approval follow the own diff.** On a new child revision, a file stays viewed if its own diff is unchanged, and an approval stays only if the whole own diff is unchanged. Whether the rebase was clean doesn't matter.
- **Land bottom first.** Approving a child ("Approve, merge after app#41") parks it in *merging*. Once the parent has merged and the child is up to date with it, the hub retargets the child to the target and merges it. Children that aren't approved are retargeted too and keep their state.
- **Conflicts.** Children are test-merged against the target like any item. A conflict their parent also has is reported on the parent only.
- **Parent abandoned.** Children get `parent.abandoned`, lose any approval and go back to their agent. The agent rebases them and moves them with `mg submit --on <other item>`, or `--on none` to put them onto the target.
- **GitHub.** A child's PR targets its parent's branch. The hub never deletes a branch that open children target, so GitHub never closes their PRs.

## 6. Agent interface (CLI)

The CLI is the universal integration surface. Agents run every command, including setup; the operator talks to their agent instead.

Command design rules:

- **Never prompt.** Every input is a flag. A command that can't proceed without a decision exits with code 2 and returns `{"needs_decision": {"question": …, "options": […], "default": …}}`. The agent turns that into a question for the operator and re-runs with the chosen flag.
- **Every setting has a command to change it later.** Nothing chosen during setup is permanent.
- `--json` everywhere, stable exit codes, and `mg doctor` to verify the whole setup and print fixes.

### 6.0 Setup and configuration

```
mg hub start [--foreground] [--port N] | stop | status  start installs a user service; stop removes it
mg hub set listen local|tailnet
mg hub set attachment-ttl <duration>                     How long a closed item keeps its demos (default 2d)
mg hub devices | allow <device> | deny <device>         Tailscale devices allowed to connect

mg project add [PATH] [--name NAME] [--backend github|local] [--github OWNER/REPO]
               [--target BRANCH] [--languages LIST|none]  PATH in a linked worktree registers its main repo
mg project set <project> <key> <value>                   backend, target, upstream, github, bots, …
mg project show [<project>]                              Default: the project at the current directory
mg judge set none|jev|command [--model M] [--command CMD] [--threshold X] [--order guide|judge] [--project P]
mg judge show [<project>]                                The judge, and the revisions it failed on
mg lang list [--available]                               Syntax highlighting language packs (hub only)
mg lang add <name>... | --detect [<project>] | --from PATH
mg lang remove <name> | update

mg connect <hub> | --local                               On a worker: point the CLI at a hub, or back
mg init                                                  In any clone: add the `mergegate` remote
mg doctor                                                Check hub, service, auth, language packs, skill, remote, GitHub access
mg skill install [--dir PATH]                            Write the agent skill and print where (§11)
```

`MERGEGATE_HOME` selects the data directory (default `~/.mergegate`) for both the hub and the CLI, which finds the hub's address there. A development build can run next to the installed one without sharing state.

`mg hub start` for the default home runs the hub as a user service: a launchd agent (`dev.mergegate.hub`) on macOS, a systemd user unit (`mergegate-hub.service`) on Linux. It starts at login and is restarted after a crash or failed start, but not after `mg hub stop`, which also removes the unit. The unit runs the `mg` that wrote it with that shell's `PATH`; `mg doctor` warns when it runs another. Where no service manager is usable (no systemd user session) the hub starts detached with a warning. A hub with `MERGEGATE_HOME` set, or started `--foreground`, never touches a unit, so a development build or a test can't replace the installed hub's.

A repo with a GitHub remote is asked which backend to use (a `needs_decision`, defaulting to `github`); one without lands on `local`. The GitHub repository is origin's if origin is on github.com, else the only github.com remote, else a `needs_decision`; `--github` names it and implies `--backend github`. The hub checks that the operator can push to it before registering. A local project keeps its GitHub repository, so switching backends back and forth needs no flags.

`attachment-ttl` takes `30s`, `15m`, `12h` or `2d`; `0s` drops a closed item's attachments as it closes. The hub prunes on start, hourly, after an item closes and when the TTL changes, removing the rows and the files no other row uses.

Changing a project's backend, GitHub repository or target applies to new items. Open items keep their settings until they close, and the command lists them in its `needs_decision` so the operator knows.

### 6.1 Authoring agent commands

```
mg submit [--title T] [--base main] [--on <item>|none] [--guide FILE|-] [--attach FILE]...
          [--priority p0|p1|p2|p3] [--note TEXT] [--json]
    Push HEAD to the hub and create the item, or add a revision if this branch already has an open item.
    --on: stack the item on an open item (§5.1); `none` moves a stacked item back onto the target.
    --guide: the agent's reviewer guide, Markdown: the intent, risk areas and self-review notes, then a
    `## Files` list of "- `path` — reason" bullets in the order to review them (§7.2). A revision
    submitted without one keeps the last; paths it can't place come back as warnings.
    --attach: demo screenshots / recordings.
    --priority/--note: initial priority, passed down from the operator agent's task brief;
    only the submit that opens the item sets them (§10).

mg wait <item> [--for any|operator] [--timeout 540] [--since CURSOR] [--json]
    Block until there are unacknowledged events for this item.
    exit 0   events returned (--json: the item, its state, the events and the next cursor)
    exit 75  timeout with no events — call again
    exit 3   item closed (merged / abandoned)
    --for operator: only an event from the operator ends the wait, which then returns every
    unacknowledged event with it, so acknowledging never skips one. --timeout 0 waits until an event.
    External comments come in batches, not a trickle: the hub holds an item's GitHub comments until
    they have been quiet for 60s and every bot in the project's `bots` has posted or edited a
    comment GitHub dates at or after the newest push (or 20 minutes have passed since it), and
    sends them 20 minutes after the first regardless.
    A read that returns other events takes the held comments with it.

mg feedback <item> [--since CURSOR] [--json]     Non-blocking version of wait.
    The hub keeps a delivered cursor per item: wait and feedback return the events after it, and the
    CLI acknowledges them once printed, so an event arrives at least once. --since replays from any
    cursor without acknowledging.
mg reply <comment> --status addressed|rejected|question [--body TEXT]
    Respond to a comment (operator's or external). Replies thread on the comment and the latest
    status counts; a rejection or a question needs a body. Replies live in mergegate only, where the
    operator sees them; nothing is posted to GitHub.
mg sync <item>                                   Refetch the item's pull request comments from GitHub
                                                 and send the held ones now: the fix when polling missed
                                                 something. The UI's Sync GitHub button does the same.
mg attach <item> FILE...                         Add demo media to the latest revision.
mg abandon <item> [--reason TEXT]
mg resume <item>                                 Print a self-contained prompt (item context + unaddressed
                                                 feedback) for starting a fresh agent session.
```

`mg resume` holds the skill's rules, the item, its branch and the worktree it was last submitted from, the last review, and every comment still open with its thread. A comment is settled when the agent's latest reply is `rejected`, or `addressed` and a revision was submitted since, and the operator hasn't replied after it. It acknowledges nothing, so the old session's unread events still reach `mg wait`.

`mg submit` also takes `--wake CMD`: a command the hub runs when the item has new events from the operator, typically the agent's own "resume this session" command (§6.5). The latest submit that passes it wins, and `--wake ""` removes it.

### 6.2 Operator-agent commands

```
mg inbox [--json]                                      The review queue as the UI shows it.
mg prioritize <item> --priority p0|p1|p2|p3|none [--note "blocks auth refactor, 3 agents waiting"]
mg events [--since CURSOR] [--follow] [--json]         Hub-wide event log (merged, abandoned, …)
                                                       so the operator agent can dispatch dependent work.
                                                       Acknowledges nothing; --follow streams one JSON
                                                       object per line, from now or --since.
mg rules list | propose ... | show <id>                See §7.3.
```

### 6.3 Event shape (sketch)

```json
{
  "cursor": 193,
  "item": "app#42",
  "revision": 2,
  "type": "review.submitted",
  "source": "operator",
  "verdict": "changes_requested",
  "summary": "Token refresh races with logout; see inline.",
  "comments": [
    { "id": "c_17", "revision": 2, "path": "Sources/Auth/TokenStore.swift", "side": "new",
      "start_line": 86, "line": 88, "quote": "    store(t)",
      "body": "This can run after logout() and resurrect the session." }
  ],
  "replies": [
    { "comment": "c_12", "author": "operator", "body": "Keep the retry; drop the sleep." }
  ]
}
```

The cursor is the event's position in the item's log, and comment IDs are hub-wide. A comment anchors to a line range on one side of a file in one revision and quotes the last few of its lines, so the agent needn't open the file to know what it means. Comments are on lines only; a whole-file remark goes in the review's summary. `replies` are the operator's answers to the agent's replies on earlier comments. Only events the agent acts on enter the log; its own submits and replies don't.

Event types: `review.submitted`, `external.comments` (each comment carries `external`: its author, whether a bot, its kind (`review_comment`, `issue_comment` or `review`) and its GitHub URL; one that can't be placed on a line has no anchor, and `edited` lists those an earlier batch carried with another body), `merge.held` (the upstream's reason), `merge.cancelled`, `merge.conflict` (the target commit, the paths and the commands to rebase), `item.merged` (the squash commit and the branch, which git sees as unmerged), `item.abandoned`, `parent.updated`, `parent.abandoned`, `comment.override` (the operator overrode the agent's rejection of an external comment: the comment and the operator's reply, sent at once rather than with a review).

External comments share the operator's comment IDs, so the agent answers them with `mg reply` too. An inline one anchors on the revision whose head it was written on; a conversation comment, a review body or an inline one on a commit the hub never had sits on the item as a whole. Each GitHub reply in a thread is its own comment, an edit comes again in the next batch, and a batch keeps the bodies it sent, so an event reads the same after an edit. The operator's own comments (the account `gh` signs in as) are ignored.

A comment deleted on GitHub while the hub still holds it is withdrawn, but only once GitHub, asked for that comment by itself, answers 404: lists come a page at a time by offset, so one read while an earlier comment is deleted can skip a comment that's still there. One no batch carried and no one answered goes, with the operator's draft replies on it; one the agent already got stays as the agent got it, draft replies included, and only its held edit is dropped. A comment deleted after it went out stays as it is.

An expected bot has reviewed the newest push once GitHub dates a comment of its at or after the push: a comment by when it was posted or last edited, so a bot that keeps one summary comment counts once it edits it, and a review by its submission. When the hub saw the comment or the edit doesn't matter, so a bot's review of the revision before, read late, doesn't count.

### 6.4 Push

Pulling (`mg wait`) is the core mechanism because every agent supports it. The only push is an item's own `--wake` (§6.5), which brings back the agent that submitted it. The hub runs no hub-wide hooks; a harness that reacts to events across items follows `mg events --follow` (§6.2).

### 6.5 Waiting and token cost

No model is involved in polling:

- **The hub polls GitHub.** It's plain code in the daemon and costs no tokens.
- **`mg wait` is one blocking command.** While it blocks, the agent isn't generating anything. Tokens are spent only when it returns, either with events (work the agent has to do anyway, with GitHub comments batched by the hub) or on a timeout (one short line).

The agent picks the cheapest way to wait that its environment supports. The skill describes these by capability, not by agent name:

| If the agent can… | It does | Tokens while waiting |
|---|---|---|
| run a command in the background and be notified when it exits | `mg wait --timeout 0` in the background | none |
| be resumed by a shell command | `mg submit --wake "<resume cmd>"`, then ends its turn | none |
| neither | loops `mg wait --timeout N`, N just under its tool-call limit | one small turn per timeout |

A wake runs as `sh -c CMD` in the worktree of the item's latest submit, with `{item}` replaced and `MG_ITEM`, `MG_CURSOR` and `MERGEGATE_HOME` set, in its own process group. It runs when the item has events from the operator or GitHub its agent hasn't acknowledged (its own, like an `mg abandon`, never wake it), but not while a read of the item is in flight, not for events a read already returned, and only one at a time per item, looking again when the command exits. Each event wakes at most once, across hub restarts. The command's stderr goes to `wakes/<item>.log` in the hub's home, not a pipe, since the session it starts may outlive the hub; the hub logs a failed command and doesn't retry. `mg resume` shows the wake an earlier session left, so a new one replaces it. GitHub comments wake only once their batch is sent, and once workers connect from other machines (PR 19) they can't set one or move where one runs (§4).

## 7. File triage

### 7.1 Tiers

| Tier | UI treatment |
|---|---|
| `review` | Every file not hidden by a rule, ordered from most to least important, each with a one-line reason. Low-value files sink to the bottom. |
| `trusted` | Hidden behind a count ("14 trusted files"), auto-marked viewed, one click to expand. |

Two groups only. There's no middle "skim" state: importance is expressed by order within `review`, which matches how the operator works today (review it, or mark it viewed).

### 7.2 Resolution order — "only rules can hide"

1. **Built-in guards** (always `review`): changes to a `.gitattributes`, which can mark files generated, and any file the operator commented on in an earlier revision that changed again.
2. **Rules**, first match wins. They are deterministic and explainable ("trusted by rule R7: `**/*.xcstrings`").
3. **Signals that can rank but never hide.** The authoring agent's guide, the judge and structural heuristics can:
   - order files within `review`, most important first,
   - attach a one-line reason,
   - **promote**, the judge only, a rule-trusted file to `review` when it is confident the file needs a look (0.8 by default, so it isn't noisy). A promoted file says "promoted by the judge" beside its rule and isn't counted as viewed.

   They can never move a file to `trusted`.
4. Unmatched files default to `review`.

The files to review are ordered:

1. Files the judge scores 80 or more of 100, highest first. One that rose above where the guide would have put it says "raised by the judge".
2. The guide's `## Files` list, in its order. The guide alone can neither hide nor promote a file.
3. The rest by the judge's score, a file it skipped counting as 50, then the heuristics, then path.

That is the default, `--order guide`. With `mg judge set … --order judge` the operator, or their agent, lets the judge lead instead: its scores order every file to review, a file it skipped again counting as 50, then the guide's list breaks ties, then the heuristics and path. Nothing is "raised by the judge" then, since the judge places every file; until it has scored the revision, the order is the guide's.

The trusted files come last, in path order. The heuristics read only paths, modes and the diff, with no parse: lockfiles, generated files, tests, docs and pure deletions sink; migrations, changed declarations (a function, type or export), config, symlinks, submodules and mode changes rise. A file nothing else explains shows the heuristic's hint ("ranked low: tests"). Promotion and order are applied when the item is read, so a rule change re-sorts without a new judge call.

The asymmetry matters: the authoring agent grades its own work, and the judge can be wrong. Hiding a file is therefore always the result of a rule the operator approved.

### 7.3 Rules are configured by agents and approved by the operator

Rules live on the hub, per project (`~/.mergegate/projects/<p>/rules.toml`). They're personal, hand-editable, and agent-editable through the CLI:

```
mg rules propose --tier trusted --path '**/*Tests.swift' --reason "test-only; agent runs them before submit"
mg rules propose --tier review  --path '**/*View.swift' --diff-matches 'NavigationStack|@State' \
                 --reason "views are trusted except navigation/state changes"
```

- **Tightening** (anything toward `review`) applies immediately.
- **Loosening** (anything toward `trusted`) waits for operator approval: an inbox badge, one-click approve with a preview of which past files it would have hidden.
- **The rules screen is per project,** titled "app rules", and the review screen's Trusted group links to it, so the applied rules are a click away even with nothing waiting. There are no hub-wide rules.

- **Approval happens only in the UI.** Agents propose a rule when the operator asks, never on their own. A proposed `review` rule goes first and a `trusted` one last; reordering is a hand edit.
- **A hand edit that loosens waits too** (tech-stack §7): one that only adds `review` rules or removes `trusted` ones applies, anything else waits as one pending change that approve accepts and reject reverts in the file. An invalid file is ignored, and its error shows in the UI and `mg rules list`.
- **The preview** runs once, when a change arrives, over the project's open items and its last 20 merged ones.

Matchers (v1): path glob (`*` stays within a directory), change kind (added / deleted / renamed / generated / modified), changed-line count (added + removed), diff-content regex over the added and removed lines. A rule matches when all its matchers do. Generated means `linguist-generated` as the item's base tree sets it. A matcher that can't be sure, as with a rename's two paths or a binary diff, gives whichever answer keeps the file in `review`. So no `trusted` rule matches a file whose mode or type changed, a symlink or a submodule: no matcher reads what changed there. Rules are ordered, each has an ID (`R<n>`, never reused) and a reason that shows in the UI. When the approved rules change, every open item is triaged again.

### 7.4 Judge providers

Interface: `judge(change) → [{path, importance, reason, promote}]`. The judge sees the whole change at once: the item's title, the guide, every changed file (path, kind, +/−, tier, generated, modes) and the diffs, since a file's importance depends on what else changed. `importance` is 1–100, so files rarely tie. `promote` is the confidence, 0–1, that a rule-trusted file needs a look; it can only move a file to `review` (§7.2), there's no way to suggest `trusted`.

| Provider | Notes |
|---|---|
| `none` (default) | Rules, the guide and the heuristics. Fully functional. |
| `jev` (TypeSafe AI) | Very fast and cheap, with calibrated confidences, so the threshold is meaningful. The only API provider; its key is `TYPESAFE_API_KEY` in the hub's environment, read on each call. Pinned to `jev-1.13.0` unless `--model` says otherwise. Each file gets a 10-level score (0–9, mapped onto 1–100) and each trusted one a yes/no "needs a look", whose probability is `promote`; a file a rule trusts only after the call isn't asked about, so Jev can't promote it. 429 and 529 retry with backoff. A change too large for one call is split into chunks of whole files grouped by directory, each with the full file map; a file too large alone, or binary, goes unjudged, never truncated. |
| `command` | Any other model (`claude -p`, `codex exec`, Ollama): `sh -c CMD` in the project's repo, in its own process group, killed after five minutes. |

The command reads the change as JSON on stdin and prints its verdicts on stdout. Fields are only ever added:

```
stdin:  {"item": "app#42", "revision": 2, "title": "…", "guide": "…" | null,
         "files": [{"path", "old_path", "change": "added|deleted|renamed|modified",
                    "additions", "deletions", "tier": "review|trusted", "generated",
                    "old_mode", "new_mode": "100644|100755|120000|160000" | null,
                    "diff": "unified hunks" | null}]}
stdout: {"files": [{"path", "importance": 1-100, "reason"?: "one line", "promote"?: 0-1}]}
```

The judge is per project and off until the operator picks one, since it sends their code to a model. It runs in the background, like highlighting: a submit or `mg judge set` wakes it, it judges the newest revision of every open item that lacks a verdict from the project's judge, and the UI is pinged when verdicts land; a submit never waits. Verdicts are kept per (revision, judge), where the judge is its provider and model or command; the threshold and the order apply on read. A failed call is logged once and shown by `mg judge show` until the next `mg judge set` or hub start, which retry it; meanwhile the item reads as with no judge.

## 8. Backends

| | `github` | `local` |
|---|---|---|
| Upstream | GitHub remote | The operator's repo on the hub machine |
| Code transport | worker → hub staging → hub pushes the branch to GitHub | worker → hub staging |
| PR | Hub opens/updates the PR (operator's `gh` token, §4) | none |
| External comments | Hub polls PR comments (bots and humans) → `external.comments` events | n/a |
| Operator comments | Local only | Local only |
| Agent replies | Local only, shown next to each comment | Local only |
| Merge | Squash through the GitHub merge API | Squash in staging, push to the operator's repo (`updateInstead`) |
| Stacked item | PR targets the parent's branch; retargeted to the target after the parent merges | Merged in staging after its parent |

A PR merged, closed or pushed to on GitHub directly isn't picked up yet: the item stays open, and its next submit stops at the moved branch.

The only things mergegate writes to GitHub are branches, PRs and merges. Agent replies stay in mergegate, which keeps the GitHub thread clean for human reviewers there.

The hub polls each open github item's pull request comments every 30s, with ETag-conditional requests, and `mg sync` refetches on demand. Webhooks would need a public endpoint, which conflicts with local-only.

## 9. UX

Colours and type come from the Academy Figma; light and dark follow the OS, the diff's syntax theme with them. Colour is spent on what needs the operator: a status that waits on them (a question, a rejection, a held merge, `p0`) is a solid badge, and the rest are tinted, so a screen has one or two loud things on it. Every text-on-fill pair keeps at least 4.5:1 contrast in both themes. Motion is short and says that something happened (a merge's check, a panel opening), and stops under `prefers-reduced-motion`. The UI updates live: the hub pushes what changed over SSE and the page refetches it. The tab's title names the screen and, on the inbox, how many items need you; `?` lists the screen's keys anywhere.

### 9.1 Inbox

```
┌ mergegate ─ app ───────────────────────────────────────────────────────────────┐
│ NEEDS YOU (5)                                              ⚠ 1 rule to approve │
│ p0  app#42  Token refresh on 401          r2  +212/−40 · 9 of 14 files         │
│     "blocks auth refactor — 3 agents waiting"  · worker-2 · codex: 2 → 2 fixed │
│     ⚡ conflicts with app#45                                       ▶ demo · 2h │
│ p1  app#47  Settings screen redesign      r1  +540/−120 · 6 of 8 files         │
│     claude · worker-1 · codex: running                             ▶ demo · 5h │
│ p2  …                                                                          │
│                                                                                │
│ WITH AGENTS (7)        app#44 addressing your review (r2→r3) · app#39 conflict │
│ MERGING (1)            app#40 held: main checkout has uncommitted changes      │
│ ▸ Closed today (9)                                                             │
└────────────────────────────────────────────────────────────────────────────────┘
```

- Order, within every group: explicit priority (set by the operator agent or the operator) → conflict pressure (how many open items it conflicts with, most first, since landing it settles theirs) → age, oldest first. Both items of a conflicting pair show "⚡ conflicts with" the other.
- When rules trust some files, the file count shows the ones left to review next to the total ("9 of 14 files"). That is the real cost of the item.
- A stack shows as one group, bottom item first, each child marked "↳ on app#41" (or "behind app#41" until its agent catches up) with its own to-review count.
- An item the operator reviewed on an earlier revision shows how many of its files changed since then ("2 changed since r1").
- An agent waiting on the operator, with a question or a rejected comment, puts its item in Needs you with a solid badge saying so; one with changes requested reads "waiting for rN+1".
- Items merged or abandoned today fold under *Closed today*. With nothing to review, the inbox says so and how many items the agents are working on.
- Keyboard-first: `j/k` move, `enter` open, `p` set priority and note (`0`–`3`, `n` none, `enter` or `⌘↩` saves). Rows are links, so a modifier-click opens one in a new tab.

### 9.2 Review screen

```
┌ app#42 Token refresh on 401                                                       ┐
│ r2 · Needs review · p0 · 0 of 4 reviewed · 14 trusted  [since r1 ▾] [▶ Demo (3)]  │
│                                                       [Conversation 3] [⋯]        │
│ ▸ Intent  Retries once on 401 using the refresh token…                            │
├─ Files ───────────────────────┬─ Diff ────────────────────────────────────────────┤
│ REVIEW                        │ Sources/Auth/TokenStore.swift ──────────────── +3 │
│ ☐ TokenStore.swift            │  85   func refresh() async throws {               │
│   ↳ core logic, logout race   │  86 +   guard !isLoggingOut else { return }       │
│ ☐ APIClient.swift             │  87 +   let t = try await api.refresh(…)          │
│   ↳ retry policy              │  88 +   store(t)                                  │
│ ☐ TokenStoreTests.swift       │     ┌ codex-bot ─────────────────────────────────┐│
│ ☐ Localizable+Plural.swift    │     │ store() can run after logout completes.    ││
│ TRUSTED (14) auto-viewed ▸    │     │ agent: rejected r2 — "guarded on line 86"  ││
│                               │     │ [Accept rejection] [Override — must fix]   ││
│                               │     └────────────────────────────────────────────┘│
│                               │  ⋯ 40 unmodified lines                            │
│                               │ Sources/Net/APIClient.swift ────────────── +12 −4 │
│                               │  31   func send(_ r: Request) async throws {      │
├───────────────────────────────┴───────────────────────────────────────────────────┤
│ 2 draft comments      [Send comments ⌘↩]  [Request changes]  [Approve & merge a]  │
└───────────────────────────────────────────────────────────────────────────────────┘
```

- **The header** is two lines: the title, then the revision, state, priority, how many of the files to review are reviewed and how many are trusted, with the screen's controls at the end. Rare actions (Fetch GitHub comments, Abandon…) sit under `⋯`, out of the line the operator reads. A closed item says so under the header ("Merged into main" or "Abandoned") with the item that needs the operator next.
- **The URL keeps the cursor** (`?file=…&line=…`), so a reload or a shared link opens on the same line; the review's summary and an abandon's reason are kept per tab until sent, and leaving with them unsent asks first.
- **The diff is one continuous scroll, like GitHub's:** every file in order under a sticky header, the file list beside it following the scroll. Three lines of context show around each hunk, and the gaps between hunks expand; a file whose one hunk holds all of it offers nothing more. Binary files and files over 20k changed lines show a one-line placeholder. A file's header names what its lines can't show: a changed mode or type ("Made executable (mode 100644 → 100755)"), a symlink, a submodule's commits. A viewed file collapses to its header and counts toward the viewed total.
- **Revision selector** offers *All changes* (against the merge-base) and *Since rK* for each earlier revision, defaulting to since the last revision you reviewed, or all changes when that is the newest or there's none. A closed item shows all changes only.
- **Since rK shows the interdiff only,** over a moved target: rK is replayed onto the newest revision's base and diffed against the newest, so what the target changed meanwhile drops out. A file the replay conflicts on can't be told apart from the target's changes, so it shows its whole own diff, marked as such. Files unchanged since rK fold away under the file list and can still be marked viewed; a file rK changed that the newest no longer does shows as *dropped*, and one rK didn't have, or the newest deletes, as *added* or *deleted* since rK. The old side is a replay no revision has, so only the new side takes comments. Interdiffs from every earlier revision are computed when a revision is submitted, so switching is a read.
- **Viewed marks follow a file while it's unchanged:** marking or unmarking it on rK applies to every later revision where it hasn't changed since rK, then and when a later revision arrives.
- **External comments (bots and humans) show inline with the agent's response** (addressed in rN / rejected + reason / open), marked with their author, a Bot badge, a link to GitHub and whether they are still held from the agent; one on no line is in the Conversation list. While the agent's rejection is the latest reply you can accept it, which only records your assent, or override it with an optional note, which reopens the thread and reaches the agent at once as `comment.override`. Neither goes to GitHub. *Fetch GitHub comments*, under `⋯`, refetches the pull request's comments and sends the held ones. External review runs in parallel to yours: the agent decides validity and you audit those decisions.
- **Stacked items** show the stack in the header (`app#41 ▸ app#42● ▸ app#43`) to move between its items. The diff is against the parent revision the item is built on.
- **Intent** under the header: the agent's guide, folded to its first line so the diff keeps the screen. Open, a little Markdown renders (headings, lists, code, bold), all as text; its `## Files` section is left out, since the file list carries it. Each file to review carries the guide's reason for it, the judge's score and reason ("raised by the judge 92/100: …"), and each trusted or promoted one the rule ("trusted by R3: `glob` — reason", "promoted by the judge; trusted by R3: …"); a file with none of these shows its heuristic hint.
- **Demo chip** in the header, `▶ Demo (N)`, for the screenshots and recordings attached with `mg submit --attach` or `mg attach`, so the diff keeps its height. A click opens them full size, videos with controls; ←/→ step through them and Escape closes the viewer. Attachments belong to a revision: the chip shows the newest revision's, or, while it has none, the newest earlier revision's, labeled "from rN", so a small fix needn't be recorded again; with the revision selector it follows the selected revision. Only PNG, JPEG, GIF, WebP, MP4 and WebM up to 100 MB, typed by their bytes, since the hub serves them from its own origin. GitHub items keep theirs in mergegate: GitHub has no upload API. An attach updates the screen and the inbox's `▶ demo` marker live; it tells the agent nothing.
- **Pending review model** (like GitHub): comments are drafts until you submit. Submit sends one `review.submitted` event. Drafts live on the hub, so a reload or a second tab keeps them, and a sent comment can't be edited. A review needs a summary, a comment or a draft reply. Send comments (`⌘↩` from the summary) notifies the agent and leaves the item in Needs you; Request changes moves it to Changes requested. A review written against a revision that is no longer the newest is refused, so it can't land on code the operator hasn't seen.
- **Approve & merge** asks first, saying what will happen (squash rN onto main, or squash-merge the pull request on GitHub and delete its branch), how many files aren't reviewed yet without blocking on them, and which open items it conflicts with; Enter merges and Escape backs out. It sends the drafts as an `approved` review, then squashes the revision onto the target. A held merge says why in plain words under the header, git's own output under *Details*, with Retry and a red Cancel; a conflict shows its paths there while the agent rebases. The inbox row carries the same badge.
- **Replies thread under each comment:** the agent's, with their status (addressed in rN, rejected, question), and yours, drafted like comments and sent with the next review. "Addressed in rN" is the first revision submitted after the reply. Replies show as they arrive: the UI's stream carries `item.changed` pings and the screen rereads the item's conversation, as it does on reconnect.
- **Conversation list** in the header: every comment as `path:line @rN`, where it was written, with its latest status and replies. One the diff shows sums up there with *Show in diff*; the rest are answered there.
- **Threads follow their lines** onto later revisions, the operator's and GitHub's alike: a comment from rK shows on the revision on screen at the lines its own became, through a line diff of its file between the two revisions (base to base for the old side). One whose lines changed sits on the lines that replaced them, marked *changed since rK*. One whose lines were deleted outright, that the diff doesn't show, whose file was renamed (files match by path), or that is in a binary or too large file is *outdated* and shows only in the Conversation list. The agent still sees each sent comment where it was sent.
- **New revision mid-review:** a banner says "rN submitted" and offers to show changes since the revision on screen. Nothing moves until you switch: comments and reviews go to the revision on screen, and Approve waits for the newest. A review takes the drafts from earlier revisions with it, anchored on its own revision. Those on unchanged lines move by themselves and show at their new lines marked *from rK*. One whose lines changed or went is flagged and holds Comment, Request changes and Approve until you keep it on the lines that replaced its own, re-place it on lines you pick (`c` or the gutter's `+`), or delete it.
- Keyboard: `n/p` file, `]/[` hunk, `j/k` line, `v` viewed (and on to the next file), `Escape` closes the open editor, then the Conversation list or a re-placing, then goes back to the inbox, `c` comment on the line, `r` the review's summary, `a` approve, `P` set priority (`p` is taken).

## 10. Prioritization

mergegate doesn't try to know the plan. The operator agent does.

- **Initial, at submit:** the operator agent includes priority and a "why" note in the task brief it dispatches. The authoring agent passes them to `mg submit --priority --note`, so items arrive already prioritized. Only the submit that creates the item sets them; a later one warns and keeps what's there, so an agent repeating its brief can't undo the operator.
- **Updates, any time:** `mg prioritize` (operator agent, when the plan changes) or the priority picker in the UI (operator). Priority and note are written together, the latest write wins, and the note is shown in the inbox. No priority sorts after p3.
- **Derived signals** for tie-breaking and display: pairwise conflicts between open items (§5), items stacked on it, age, review-tier size, open external comments.
- **Optional judge pass** over the inbox: rank items given notes and signals. This is never needed for correctness.
- `mg events --follow` lets the operator agent react to merges and start dependent work.

## 11. Agent integration: one skill

mergegate ships a single Agent Skill: a folder with a `SKILL.md`, the format Claude Code, Codex, opencode and Cursor all load natively. There are no per-agent variants.

```
mergegate/
  SKILL.md            when to use it, the core loop, the rules
  references/
    setup.md          set up the hub, projects, workers and judge; which decisions to ask the operator
    authoring.md      submit, wait, reply; guide format; demos
    operator.md       priorities, the event stream, rules (lands with their commands)
    waiting.md        choose how to wait by capability (§6.5)
```

- The skill names only commands that exist, and a test keeps the two in sync both ways; each command lands in the skill with the PR that adds it.
- `mg skill install` writes the folder matching the binary (to `$MERGEGATE_HOME/skill/mergegate`, or `--dir PATH`) and prints where; it isn't agent-specific, so each agent copies or links it into wherever it loads skills from. It replaces the whole folder, so files a newer build dropped go too.
- Install also asks the agent to settle with its operator what to do with a branch after its item merges: the squash leaves it unmerged to git. The skill's default is to detach, then `git branch -D` it.
- The skill covers setup too, so "set up mergegate" is a request to an agent, not a manual.

Core rules in `SKILL.md`: never `git push` or `gh pr create`; use `mg submit` with a guide, demo, and the priority/note from the task brief; wait for feedback the cheapest way available; reply to every comment with `addressed`/`rejected` + reason; evaluate external feedback critically before acting; when a command returns `needs_decision`, ask the operator.

## 12. Decisions log

| # | Question | Decision |
|---|---|---|
| 1 | Local backend: where do merges land? | On the target branch of the operator's repo on the main machine (§4). |
| 2 | GitHub branch protection / self-approval | Not a constraint. |
| 3 | CI / merge when green | No CI; approve = merge (a stacked item right after its parent). CI status may be shown if present, nothing waits on it. |
| 4 | Stacked items | Yes: `mg submit --on <item>`. Each item shows its own diff and lands after its parent. Agents keep children current by rebasing; the hub never writes to item branches (§5.1). |
| 5 | Mirror agent replies to GitHub | No. Shown in mergegate only, for bot and human external comments alike. |
| 6 | Who sets priority | Both: the authoring agent at submit (from the operator agent's brief); the operator agent or operator updates it via `mg prioritize` / UI. |
| 7 | Who runs setup | The operator's agent, end to end. Commands never prompt; decisions come back as `needs_decision` for the agent to ask. Every setting can be changed later. |
| 8 | Worker auth | Tailscale identity: same-owner devices trusted automatically, others via `mg hub allow`. No tokens: every machine is on the operator's tailnet. |
| 9 | Agent integration | One generic skill. Each agent waits with its best native mechanism, chosen by capability. |
| 10 | File groups | Two: `review` (ordered by importance) and `trusted` (hidden, rules only). No skim tier. |
| 11 | Merge strategy | Squash, on both backends: one commit per item, authored by the operator (§4). |

No open UX questions remain. Tech stack decisions are in [tech-stack.md](tech-stack.md).
