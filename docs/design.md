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
  - `github`: the hub pushes the branch to GitHub under the agent's branch name and opens or updates the PR: titled as the item (and retitled with it), with an empty body, never a draft. The push leases the branch from where the hub last pushed it (staging remembers that per branch), so anything pushed there by someone else stops the submit (`branch_moved`) instead of being overwritten, while a retry after a submit that failed partway, or a later item reusing a merged item's branch name, goes through. An open item whose branch is gone from GitHub, as closing its PR there may leave it, stops at `branch_gone` rather than pushing to a PR GitHub won't reopen. An existing open PR from the branch into the item's target is adopted. A submit waits for GitHub; if GitHub can't be reached, the submit fails and nothing is recorded. Merging test-merges in staging first (a conflict is a `merge.conflict`, as on `local`), then squash-merges through the API at the approved head only, deletes the branch and re-fetches the target. A retried merge that GitHub already made is recorded as merged. A merge GitHub refuses (branch protection, required checks) is held with GitHub's reason. Abandoning closes the PR and deletes the branch, and waits for GitHub too.
  - `local`: the hub squashes the item in staging (`git merge-tree`, no working tree needed), then pushes that one commit to the operator's repo with `receive.denyCurrentBranch=updateInstead` semantics. If the target branch is checked out and clean, the checkout fast-forwards in place. If it has uncommitted changes, the merge is held and the UI says why. Nothing is ever force-updated.
- **Every item lands as one squash commit** on the target, on both backends: authored and committed by the operator (their repo's `user.name` and `user.email`), with the subject `<title> (app#42)`. Nothing in it names an agent. On `github` GitHub writes the commit, so its author is the operator's GitHub account (the one `gh` is signed in as) and its committer is GitHub. Since git doesn't see the item's branch as merged afterwards, agents delete it with `git branch -D`.
- **Workers take the target from the hub.** `git fetch mergegate` returns the latest target (mirrored from the upstream), so new worktrees start from what was actually merged. The hub re-fetches the target from the upstream before every fetch and submit, so a worker's push to it doesn't stick.
- **Every revision is pinned.** The hub keeps each revision's head at `refs/mergegate/items/<n>/r<rev>`, hidden from workers' pushes and fetches, so a force-push to the branch can't drop the commits an older revision points at.
- **Diffs are computed on the hub** from staging: the merge-base against the target (or the parent revision for a stacked item, §5.1), and interdiffs between revisions.

### Multi-machine & security

- By default the hub listens on `127.0.0.1` only. `mg hub set listen tailnet` also binds it to the Tailscale interface.
- **Tailscale identity instead of tokens.** For each tailnet connection, the hub asks the local Tailscale daemon who is calling (`whois`). Devices owned by the same Tailscale user as the hub are trusted automatically. Other devices need `mg hub allow <device>`. There's nothing to copy between machines, so an agent can connect a worker end to end (F2).
- Fallback for networks without Tailscale: `mg hub token create` on the hub, `mg connect <url> --token T` on the worker.
- `mg wait` connections are resumable via an event cursor, so a dropped connection never loses feedback.
- **The hub signs in to GitHub as the operator's `gh` does.** It runs `gh auth token --hostname github.com` and keeps the token in memory only, never on disk or in logs, reading it again after GitHub turns it down. Git gets it as an HTTP header through the environment, scoped to the repository's URL, so it never shows in the process list or reaches another remote. A logged-out `gh` is an error whose fix is `gh auth login`. Only github.com is supported.
- Only a worker on the hub's machine may set `mg submit --wake`: the hub runs whatever command it is given. The hub decides by the connection, not by anything the client sends: a `--wake` that arrives over the tailnet or with a token is refused with an error, so the agent knows to wait with `mg wait` instead. Since a wake runs in the worktree of the item's latest submit, a submit from another machine clears the item's wake, so a remote worker can't choose the directory a trusted command starts in.

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
- Conflicts are detected early: the hub test-merges every open item whenever the target moves, and sends conflicts to the authoring agent before the operator gets to the item.
- **Closing an item prunes its diffs.** When an item merges or is abandoned, the hub drops its revision pins (a merged item keeps its newest revision's) and deletes the cached file diffs no open item's revision still uses (the cache is keyed by blob pair, so items and revisions share entries). Its file list and line counts stay. Opening a merged item diffs its files again on read, without caching them, since its blobs live on in the target's history and that pin, which a squash needs when it auto-merged a file the target also changed; an abandoned item's blobs may be garbage-collected, so it shows the file list and counts with a note that the diff is gone.

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
mg hub start [--foreground] [--port N] | stop | status
mg hub set listen local|tailnet
mg hub devices | allow <device> | deny <device>         Tailscale devices allowed to connect
mg hub token create                                      Fallback auth for non-Tailscale networks

mg project add [PATH] [--name NAME] [--backend github|local] [--github OWNER/REPO]
               [--target BRANCH]                         PATH in a linked worktree registers its main repo
mg project set <project> <key> <value>                   backend, target, upstream, github, bots, …
mg project show [<project>]                              Default: the project at the current directory
mg judge set none|jev|anthropic|openai-compatible|command [provider options]
mg lang list [--available]                               Syntax highlighting language packs (hub only)
mg lang add <name>... | --detect [<project>] | --from FILE
mg lang remove <name> | update

mg connect <hub>                                         On a worker: point the CLI at a hub
mg init                                                  In any clone: add the `mergegate` remote
mg doctor                                                Check hub, auth, remote, GitHub access
mg skill install [--dir PATH]                            Write the agent skill and print where (§11)
```

`MERGEGATE_HOME` selects the data directory (default `~/.mergegate`) for both the hub and the CLI, which finds the hub's address there. A development build can run next to the installed one without sharing state.

A repo with a GitHub remote is asked which backend to use (a `needs_decision`, defaulting to `github`); one without lands on `local`. The GitHub repository is origin's if origin is on github.com, else the only github.com remote, else a `needs_decision`; `--github` names it and implies `--backend github`. The hub checks that the operator can push to it before registering. A local project keeps its GitHub repository, so switching backends back and forth needs no flags.

Changing a project's backend or target applies to new items. Open items keep their settings until they close, and the command lists them in its `needs_decision` so the operator knows.

### 6.1 Authoring agent commands

```
mg submit [--title T] [--base main] [--on <item>|none] [--guide FILE|-] [--attach FILE]...
          [--priority p0|p1|p2|p3] [--note TEXT] [--json]
    Push HEAD to the hub and create the item, or add a revision if this branch already has an open item.
    --on: stack the item on an open item (§5.1); `none` moves a stacked item back onto the target.
    --guide: the agent's reviewer guide (intent, risk areas, suggested file order, self-review notes).
    --attach: demo screenshots / recordings.
    --priority/--note: initial priority, passed down from the operator agent's task brief.

mg wait <item> [--for any|operator] [--timeout 540] [--since CURSOR] [--json]
    Block until there are unacknowledged events for this item.
    exit 0   events returned (--json: the item, its state, the events and the next cursor)
    exit 75  timeout with no events — call again
    exit 3   item closed (merged / abandoned)
    --for operator: only an event from the operator ends the wait, which then returns every
    unacknowledged event with it, so acknowledging never skips one. --timeout 0 waits until an event.
    External comments are debounced (quiet window, default 60s) so the agent gets a batch, not a trickle.

mg feedback <item> [--since CURSOR] [--json]     Non-blocking version of wait.
    The hub keeps a delivered cursor per item: wait and feedback return the events after it, and the
    CLI acknowledges them once printed, so an event arrives at least once. --since replays from any
    cursor without acknowledging.
mg reply <comment> --status addressed|rejected|question [--body TEXT]
    Respond to a comment (operator's or external). Replies thread on the comment and the latest
    status counts; a rejection or a question needs a body. Replies live in mergegate only, where the
    operator sees them; nothing is posted to GitHub.
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
mg prioritize <item> --priority p0|p1|p2|p3 [--note "blocks auth refactor, 3 agents waiting"]
mg events --follow [--json]                            Hub-wide event stream (merged, abandoned, …)
                                                       so the operator agent can dispatch dependent work.
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

Event types: `review.submitted`, `external.comments` (each comment carries `author` and `kind: bot|human`), `merge.held` (the upstream's reason), `merge.cancelled`, `merge.conflict` (the target commit, the paths and the commands to rebase), `item.merged` (the squash commit and the branch, which git sees as unmerged), `item.abandoned`, `parent.updated`, `parent.abandoned`, `comment.override` (operator overrode an agent's rejection of an external comment).

### 6.4 Push extension point

Pulling (`mg wait`) is the core mechanism because every agent supports it. For harnesses that want to start an agent when something happens, the hub runs configured hooks:

```toml
[[hooks.on_event]]
match = { type = "review.submitted" }
run = "my-harness resume --item {item}"
```

### 6.5 Waiting and token cost

No model is involved in polling:

- **The hub polls GitHub.** It's plain code in the daemon and costs no tokens.
- **`mg wait` is one blocking command.** While it blocks, the agent isn't generating anything. Tokens are spent only when it returns, either with events (work the agent has to do anyway, batched by the debounce) or on a timeout (one short line).

The agent picks the cheapest way to wait that its environment supports. The skill describes these by capability, not by agent name:

| If the agent can… | It does | Tokens while waiting |
|---|---|---|
| run a command in the background and be notified when it exits | `mg wait --timeout 0` in the background | none |
| be resumed by a shell command | `mg submit --wake "<resume cmd>"`, then ends its turn | none |
| neither | loops `mg wait --timeout N`, N just under its tool-call limit | one small turn per timeout |

A wake runs as `sh -c CMD` in the worktree of the item's latest submit, with `{item}` replaced and `MG_ITEM`, `MG_CURSOR` and `MERGEGATE_HOME` set, in its own process group. It runs when the item has events from the operator its agent hasn't acknowledged (its own, like an `mg abandon`, never wake it), but not while a read of the item is in flight, not for events a read already returned, and only one at a time per item, looking again when the command exits. Each event wakes at most once, across hub restarts. The command's stderr goes to `wakes/<item>.log` in the hub's home, not a pipe, since the session it starts may outlive the hub; the hub logs a failed command and doesn't retry. `mg resume` shows the wake an earlier session left, so a new one replaces it. The external-comment debounce (PR 10) will apply to wakes too, and once workers connect from other machines (PR 19) they can't set one or move where one runs (§4).

## 7. File triage

### 7.1 Tiers

| Tier | UI treatment |
|---|---|
| `review` | Every file not hidden by a rule, ordered from most to least important, each with a one-line reason. Low-value files sink to the bottom. |
| `trusted` | Hidden behind a count ("14 trusted files"), auto-marked viewed, one click to expand. |

Two groups only. There's no middle "skim" state: importance is expressed by order within `review`, which matches how the operator works today (review it, or mark it viewed).

### 7.2 Resolution order — "only rules can hide"

1. **Built-in guards** (always `review`): changes to mergegate rule files, and any file the operator commented on in an earlier revision that changed again.
2. **Rules**, first match wins. They are deterministic and explainable ("trusted by rule R7: `**/*.xcstrings`").
3. **Signals that can rank but never hide.** The authoring agent's guide and the judge can:
   - order files within `review`, most important first,
   - attach reasons and flags ("touches navigation state"),
   - **promote** a rule-trusted file to `review` when its content looks risky (above a confidence threshold, so it isn't noisy).

   They can never move a file to `trusted`.
4. Unmatched files default to `review`.

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

Matchers (v1): path glob, change kind (added / deleted / renamed / generated), changed-line count, diff-content regex. Rules are ordered and each carries a reason that shows in the UI.

### 7.4 Judge providers

Interface: `classify(files, item context) → [{path, promote, rank, reason, flags, confidence}]`. `promote` can only move a rule-trusted file to `review` (§7.2); there's no way to suggest `trusted`.

| Provider | Notes |
|---|---|
| `none` (default) | Rules + authoring-agent guide only. Fully functional. |
| `jev` (TypeSafe AI) | Very fast (70–500ms) and cheap, with calibrated confidences, so the promotion threshold is meaningful. Hosted, early-access, and doesn't take images yet. Opt-in. |
| `anthropic` / `openai-compatible` | e.g. Haiku via API key, or Ollama locally. |
| `command` | Shell out to an installed agent CLI (`claude -p`, `codex exec`) with a JSON contract. |

Any hosted provider sees diff content, so it's opt-in per project. Judge results are cached per (file blob, rules version).

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

GitHub sync polls (ETag-conditional requests) at a short interval for open items. Webhooks would need a public endpoint, which conflicts with local-only.

## 9. UX

Colours and type come from the Academy Figma; light and dark follow the OS. Status shows as solid badges or tinted rows, and every text-on-fill pair keeps at least 4.5:1 contrast in both themes. The UI updates live: the hub pushes what changed over SSE and the page refetches it.

### 9.1 Inbox

```
┌ mergegate ─ app ───────────────────────────────────────────────────────────────┐
│ NEEDS YOU (5)                                              ⚠ 1 rule to approve │
│ p0  app#42  Token refresh on 401          r2  +212/−40 (68 to review)          │
│     "blocks auth refactor — 3 agents waiting"  · worker-2 · codex: 2 → 2 fixed │
│     ⚡ conflicts with app#45                                       ▶ demo · 2h │
│ p1  app#47  Settings screen redesign      r1  +540/−120 (31 to review)         │
│     claude · worker-1 · codex: running                             ▶ demo · 5h │
│ p2  …                                                                          │
│                                                                                │
│ WITH AGENTS (7)        app#44 addressing your review (r2→r3) · app#39 conflict │
│ MERGING (1)            app#40 held: main checkout has uncommitted changes      │
│ MERGED TODAY (9)                                                               │
└────────────────────────────────────────────────────────────────────────────────┘
```

- Order: explicit priority (set by the operator agent or the operator) → conflict pressure (merging this unblocks others) → age.
- "To review" counts only `review`-tier lines. That is the real cost of the item.
- A stack shows as one group, bottom item first, each child marked "↳ on app#41" (or "behind app#41" until its agent catches up) with its own to-review count.
- Keyboard-first: `j/k` move, `enter` open, `p` set priority.

### 9.2 Review screen

```
┌ app#42 Token refresh on 401 ── r1 r2● [since last review ▾] ─ p0 [Conversation 3] ┐
│ ┌ Intent (authoring agent) ─────────────────┐ ┌ Demo ───────────────────────────┐ │
│ │ Retries once on 401 using refresh token…  │ │ [▶ recording.mp4] [img] [img]   │ │
│ │ Risk: logout race. Self-review: …         │ └─────────────────────────────────┘ │
│ └───────────────────────────────────────────┘                                     │
├─ Files · 0/18 viewed ─────────┬─ Diff ────────────────────────────────────────────┤
│ REVIEW                        │ Sources/Auth/TokenStore.swift ──────────────── +3 │
│ ☐ TokenStore.swift            │  85   func refresh() async throws {               │
│   ↳ core logic, logout race   │  86 +   guard !isLoggingOut else { return }       │
│ ☐ APIClient.swift             │  87 +   let t = try await api.refresh(…)          │
│   ↳ retry policy              │  88 +   store(t)                                  │
│ ☐ TokenStoreTests.swift       │     ┌ codex-bot ─────────────────────────────────┐│
│ ☐ Localizable+Plural.swift    │     │ store() can run after logout completes.    ││
│ TRUSTED (14) auto-viewed ▸    │     │ agent: rejected r2 — "guarded on line 86"  ││
│                               │     │ [agree] [override → must fix]              ││
│                               │     └────────────────────────────────────────────┘│
│                               │  ⋯ 40 unmodified lines                            │
│                               │ Sources/Net/APIClient.swift ────────────── +12 −4 │
│                               │  31   func send(_ r: Request) async throws {      │
├───────────────────────────────┴───────────────────────────────────────────────────┤
│ 2 draft comments            [Comment]  [Request changes]  [Approve & merge]       │
└───────────────────────────────────────────────────────────────────────────────────┘
```

- **The diff is one continuous scroll, like GitHub's:** every file in order under a sticky header, the file list beside it following the scroll. Three lines of context show around each hunk, and the gaps between hunks expand. Binary files and files over 20k changed lines show a one-line placeholder. A viewed file collapses to its header and counts toward the viewed total.
- **Revision selector** defaults to "since last review": you see the interdiff only. A file that hasn't changed since you marked it viewed stays viewed.
- **External comments (bots and humans) show inline with the agent's response** (addressed in rN / rejected + reason / open). You can agree, or override a rejection. An override becomes your comment and an event for the agent. External review runs in parallel to yours: the agent decides validity and you audit those decisions.
- **Stacked items** show the stack in the header (`app#41 ▸ app#42● ▸ app#43`) to move between its items. The diff is against the parent revision the item is built on.
- **Demo panel** at the top for screenshots and recordings attached with `mg submit --attach`.
- **Pending review model** (like GitHub): comments are drafts until you submit. Submit sends one `review.submitted` event. Drafts live on the hub, so a reload or a second tab keeps them, and a sent comment can't be edited. A review needs a summary, a comment or a draft reply. Comment notifies the agent and leaves the item in Needs you; Request changes moves it to Changes requested.
- **Approve & merge** asks first, showing how many files are still unviewed without blocking on them; Enter merges and Escape backs out. It sends the drafts as an `approved` review, then squashes the revision onto the target. A held merge shows the upstream's reason under the header with Retry and Cancel; a conflict shows its paths there while the agent rebases. The inbox row carries the same badge.
- **Replies thread under each comment:** the agent's, with their status (addressed in rN, rejected, question), and yours, drafted like comments and sent with the next review. "Addressed in rN" is the first revision submitted after the reply. Replies show as they arrive: the UI's stream carries `item.changed` pings and the screen rereads the item's conversation, as it does on reconnect.
- **Conversation list** in the header: every comment as `path:line @rN` with its latest status and replies. A comment shows inline only on the revision it was written on until it is re-mapped, so older ones are answered here.
- **New revision mid-review:** a banner appears. Drafts stay anchored to the old revision and are re-mapped to the new lines on submit, or flagged when they can't be re-mapped.
- Keyboard: `n/p` file, `]/[` hunk, `j/k` line, `v` viewed (and on to the next file), `Escape` closes the open editor, then the Conversation list, then goes back to the inbox, `c` comment on the line, `r` the review's summary, `a` approve.

## 10. Prioritization

mergegate doesn't try to know the plan. The operator agent does.

- **Initial, at submit:** the operator agent includes priority and a "why" note in the task brief it dispatches. The authoring agent passes them to `mg submit --priority --note`, so items arrive already prioritized.
- **Updates, any time:** `mg prioritize` (operator agent, when the plan changes) or drag/keys in the UI (operator). The latest write wins, and the note is shown in the inbox.
- **Derived signals** for tie-breaking and display: pairwise conflicts between open items, items stacked on it, age, review-tier size, open external comments.
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
| 8 | Worker auth | Tailscale identity: same-owner devices trusted automatically, others via `mg hub allow`. Tokens only as a fallback. |
| 9 | Agent integration | One generic skill. Each agent waits with its best native mechanism, chosen by capability. |
| 10 | File groups | Two: `review` (ordered by importance) and `trusted` (hidden, rules only). No skim tier. |
| 11 | Merge strategy | Squash, on both backends: one commit per item, authored by the operator (§4). |

No open UX questions remain. Tech stack decisions are in [tech-stack.md](tech-stack.md).
