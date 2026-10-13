# mergegate — system & UX design

Status: draft v0.1 · 2026-10-12 · tech stack intentionally out of scope (next session)

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
- Stacked / dependent items. Every agent works in its own worktree off the target branch.
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
| **Authoring agent** | The agent that produced the item. Submits it, waits for feedback, and addresses it. |
| **Operator agent** | The agent the operator talks to that plans and dispatches work. It knows what blocks what, so it sets item priority. |
| **Event** | Anything an authoring agent should react to: operator review, external comments, merge conflict, merged. |
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
  - `github`: the hub pushes the branch to GitHub and opens or updates the PR. Merging uses the GitHub API.
  - `local`: the hub merges in staging (`git merge-tree`, no working tree needed), then pushes the result to the operator's repo with `receive.denyCurrentBranch=updateInstead` semantics. If the target branch is checked out and clean, the checkout fast-forwards in place. If it has uncommitted changes, the merge is held and the UI says why. Nothing is ever force-updated.
- **Workers take the target from the hub.** `git fetch mergegate` returns the latest target (mirrored from the upstream), so new worktrees start from what was actually merged.
- **Diffs are computed on the hub** from staging: the merge-base against the target, and interdiffs between revisions.

### Multi-machine & security

- By default the hub listens on `127.0.0.1` only. `mg hub set listen tailnet` also binds it to the Tailscale interface.
- **Tailscale identity instead of tokens.** For each tailnet connection, the hub asks the local Tailscale daemon who is calling (`whois`). Devices owned by the same Tailscale user as the hub are trusted automatically. Other devices need `mg hub allow <device>`. There's nothing to copy between machines, so an agent can connect a worker end to end (F2).
- Fallback for networks without Tailscale: `mg hub token create` on the hub, `mg connect <url> --token T` on the worker.
- `mg wait` connections are resumable via an event cursor, so a dropped connection never loses feedback.

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

- External review runs **in parallel** with operator review, mirroring today's workflow. Items enter *needs review* immediately; external comments and the agent's responses appear as they come in.
- **Approve means merge.** There's no CI to wait for, so an approval merges right away. If the merge can't happen (conflict), the item goes back to the agent with a `merge.conflict` event. If the upstream checkout is dirty (local backend), it stays in *merging* until the operator retries.
- A new revision while the operator is mid-review never discards draft comments (§9.2).
- Conflicts are detected early: the hub test-merges every open item whenever the target moves, and sends conflicts to the authoring agent before the operator gets to the item.

## 6. Agent interface (CLI)

The CLI is the universal integration surface. Agents run every command, including setup; the operator talks to their agent instead.

Command design rules:

- **Never prompt.** Every input is a flag. A command that can't proceed without a decision exits with code 2 and returns `{"needs_decision": {"question": …, "options": […], "default": …}}`. The agent turns that into a question for the operator and re-runs with the chosen flag.
- **Every setting has a command to change it later.** Nothing chosen during setup is permanent.
- `--json` everywhere, stable exit codes, and `mg doctor` to verify the whole setup and print fixes.

### 6.0 Setup and configuration

```
mg hub start | stop | status
mg hub set listen local|tailnet
mg hub devices | allow <device> | deny <device>         Tailscale devices allowed to connect
mg hub token create                                      Fallback auth for non-Tailscale networks

mg project add [PATH] [--backend github|local] [--target BRANCH] [--upstream PATH]
mg project set <project> <key> <value>                   backend, target, upstream, bots, …
mg project show [<project>]
mg judge set none|jev|anthropic|openai-compatible|command [provider options]

mg connect <hub>                                         On a worker: point the CLI at a hub
mg init                                                  In any clone: add the `mergegate` remote
mg doctor                                                Check hub, auth, remote, GitHub access
mg skill install [--dir PATH]                            Install the agent skill (§11)
```

Changing a project's backend or target applies to new items. Open items keep their settings until they close, and the command lists them in its `needs_decision` so the operator knows.

### 6.1 Authoring agent commands

```
mg submit [--title T] [--base main] [--guide FILE|-] [--attach FILE]...
          [--priority p0|p1|p2|p3] [--note TEXT] [--json]
    Push HEAD to the hub and create the item, or add a revision if this branch already has an open item.
    --guide: the agent's reviewer guide (intent, risk areas, suggested file order, self-review notes).
    --attach: demo screenshots / recordings.
    --priority/--note: initial priority, passed down from the operator agent's task brief.

mg wait <item> [--for any|operator] [--timeout 540] [--since CURSOR] [--json]
    Block until there are unacknowledged events for this item.
    exit 0   events returned (JSON array + next cursor)
    exit 75  timeout with no events — call again
    exit 3   item closed (merged / abandoned)
    External comments are debounced (quiet window, default 60s) so the agent gets a batch, not a trickle.

mg feedback <item> [--since CURSOR] [--json]     Non-blocking version of wait.
mg reply <comment> --status addressed|rejected|question [--body TEXT]
    Respond to a comment (operator's or external). Rejections need a reason. Replies live in mergegate
    only, where the operator sees them; nothing is posted to GitHub.
mg attach <item> FILE...                         Add demo media to the latest revision.
mg abandon <item> [--reason TEXT]
mg resume <item>                                 Print a self-contained prompt (item context + unaddressed
                                                 feedback) for starting a fresh agent session.
```

`mg submit` also takes `--wake CMD`: a command the hub runs when the item has new events, typically the agent's own "resume this session" command (§6.5).

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
  "cursor": "e_0193",
  "item": "app#42",
  "revision": 2,
  "type": "review.submitted",
  "source": "operator",
  "verdict": "changes_requested",
  "summary": "Token refresh races with logout; see inline.",
  "comments": [
    { "id": "c_17", "path": "Sources/Auth/TokenStore.swift", "line": 88, "side": "new",
      "body": "This can run after logout() and resurrect the session." }
  ]
}
```

Event types: `review.submitted`, `external.comments` (each comment carries `author` and `kind: bot|human`), `merge.conflict`, `item.merged`, `item.abandoned`, `comment.override` (operator overrode an agent's rejection of an external comment).

### 6.4 Push extension point

Pulling (`mg wait`) is the core mechanism because every agent supports it. For harnesses that want to start an agent when something happens, the hub runs configured hooks:

```yaml
hooks:
  on_event:
    - match: { type: review.submitted }
      run: "my-harness resume --item {item}"
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

Rules live on the hub, per project (`~/.mergegate/projects/<p>/rules.yaml`). They're personal, hand-editable, and agent-editable through the CLI:

```
mg rules propose --tier trusted --path '**/*Tests.swift' --reason "test-only; agent runs them before submit"
mg rules propose --tier review  --path '**/*View.swift' --diff-matches 'NavigationStack|@State' \
                 --reason "views are trusted except navigation/state changes"
```

- **Tightening** (anything toward `review`) applies immediately.
- **Loosening** (anything toward `trusted`) waits for operator approval: an inbox badge, one-click approve with a preview of which past files it would have hidden.

Matchers (v1): path glob, change kind (added / deleted / renamed / generated), changed-line count, diff-content regex. Rules are ordered and each carries a reason that shows in the UI.

### 7.4 Judge providers

Interface: `classify(files, item context) → [{path, tier_suggestion, rank, reason, flags, confidence}]`.

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
| PR | Hub opens/updates the PR (operator's `gh` credentials) | none |
| External comments | Hub polls PR comments (bots and humans) → `external.comments` events | n/a |
| Operator comments | Local only | Local only |
| Agent replies | Local only, shown next to each comment | Local only |
| Merge | GitHub merge API (configurable strategy) | Merge in staging, push to the operator's repo (`updateInstead`) |

The only things mergegate writes to GitHub are branches, PRs and merges. Agent replies stay in mergegate, which keeps the GitHub thread clean for human reviewers there.

GitHub sync polls (ETag-conditional requests) at a short interval for open items. Webhooks would need a public endpoint, which conflicts with local-only.

## 9. UX

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
- Keyboard-first: `j/k` move, `enter` open, `p` set priority.

### 9.2 Review screen

```
┌ app#42 Token refresh on 401 ── r1 r2● [since last review ▾] ── p0 ───────────────┐
│ ┌ Intent (authoring agent) ─────────────────┐ ┌ Demo ───────────────────────────┐ │
│ │ Retries once on 401 using refresh token…  │ │ [▶ recording.mp4] [img] [img]   │ │
│ │ Risk: logout race. Self-review: …         │ └─────────────────────────────────┘ │
│ └───────────────────────────────────────────┘                                     │
├─ Files ───────────────────────┬─ Sources/Auth/TokenStore.swift ───────────────────┤
│ REVIEW                        │  85   func refresh() async throws {               │
│ ☐ TokenStore.swift            │  86 +   guard !isLoggingOut else { return }       │
│   ↳ core logic, logout race   │  87 +   let t = try await api.refresh(…)          │
│ ☐ APIClient.swift             │  88 +   store(t)                                  │
│   ↳ retry policy              │     ┌ codex-bot ─────────────────────────────────┐│
│ ☐ TokenStoreTests.swift       │     │ store() can run after logout completes.    ││
│ ☐ Localizable+Plural.swift    │     │ agent: rejected r2 — "guarded on line 86"  ││
│ TRUSTED (14) auto-viewed ▸    │     │ [agree] [override → must fix]              ││
│                               │     └────────────────────────────────────────────┘│
├───────────────────────────────┴───────────────────────────────────────────────────┤
│ 2 draft comments            [Comment]  [Request changes]  [Approve & merge]       │
└───────────────────────────────────────────────────────────────────────────────────┘
```

- **Revision selector** defaults to "since last review": you see the interdiff only. A file that hasn't changed since you marked it viewed stays viewed.
- **External comments (bots and humans) show inline with the agent's response** (addressed in rN / rejected + reason / open). You can agree, or override a rejection. An override becomes your comment and an event for the agent. External review runs in parallel to yours: the agent decides validity and you audit those decisions.
- **Demo panel** at the top for screenshots and recordings attached with `mg submit --attach`.
- **Pending review model** (like GitHub): comments are drafts until you submit. Submit sends one `review.submitted` event.
- **New revision mid-review:** a banner appears. Drafts stay anchored to the old revision and are re-mapped to the new lines on submit, or flagged when they can't be re-mapped.
- Keyboard: `n/p` file, `]/[` hunk, `v` viewed, `c` comment, `a` approve, `r` request changes.

## 10. Prioritization

mergegate doesn't try to know the plan. The operator agent does.

- **Initial, at submit:** the operator agent includes priority and a "why" note in the task brief it dispatches. The authoring agent passes them to `mg submit --priority --note`, so items arrive already prioritized.
- **Updates, any time:** `mg prioritize` (operator agent, when the plan changes) or drag/keys in the UI (operator). The latest write wins, and the note is shown in the inbox.
- **Derived signals** for tie-breaking and display: pairwise conflicts between open items, age, review-tier size, open external comments.
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
    operator.md       priorities, the event stream, rules
    waiting.md        choose how to wait by capability (§6.5)
```

- `mg skill install` copies it into an agent's skills directory. An agent can also install it itself, since it's just a folder.
- The skill covers setup too, so "set up mergegate" is a request to an agent, not a manual.

Core rules in `SKILL.md`: never `git push` or `gh pr create`; use `mg submit` with a guide, demo, and the priority/note from the task brief; wait for feedback the cheapest way available; reply to every comment with `addressed`/`rejected` + reason; evaluate external feedback critically before acting; when a command returns `needs_decision`, ask the operator.

## 12. Decisions log

| # | Question | Decision |
|---|---|---|
| 1 | Local backend: where do merges land? | On the target branch of the operator's repo on the main machine (§4). |
| 2 | GitHub branch protection / self-approval | Not a constraint. |
| 3 | CI / merge when green | No CI; approve = merge. CI status may be shown if present, nothing waits on it. |
| 4 | Stacked items | No; each agent works in an isolated worktree off the target. |
| 5 | Mirror agent replies to GitHub | No. Shown in mergegate only, for bot and human external comments alike. |
| 6 | Who sets priority | Both: the authoring agent at submit (from the operator agent's brief); the operator agent or operator updates it via `mg prioritize` / UI. |
| 7 | Who runs setup | The operator's agent, end to end. Commands never prompt; decisions come back as `needs_decision` for the agent to ask. Every setting can be changed later. |
| 8 | Worker auth | Tailscale identity: same-owner devices trusted automatically, others via `mg hub allow`. Tokens only as a fallback. |
| 9 | Agent integration | One generic skill. Each agent waits with its best native mechanism, chosen by capability. |
| 10 | File groups | Two: `review` (ordered by importance) and `trusted` (hidden, rules only). No skim tier. |

No open UX questions remain. Next session: tech stack (hub runtime, UI as web vs native macOS, storage, git and GitHub libraries, packaging).
