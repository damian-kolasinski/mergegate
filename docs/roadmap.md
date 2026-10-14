# mergegate — roadmap

Status: draft v0.1 · 2026-10-13 · builds [design.md](design.md) on [tech-stack.md](tech-stack.md)

The order of PRs that builds mergegate, chosen so each PR is tested end to end, delivers one complete capability, and gets mergegate reviewing its own PRs as early as possible. Remove a row once its PR merges; the code and its tests are the reference after that.

## How every PR is tested

- **PR 1 builds a `testkit` crate.** It starts the real hub binary on a random port with its own temporary `MERGEGATE_HOME`, creates real git repos and worktrees, and runs the real `mg` binary, returning its JSON output and exit code.
- **Scenario tests** run against it and are named after the flows in [user-flows.html](user-flows.html) (F1–F16), so the tests read as the spec. A flow listed on several PRs is covered in parts: each PR tests the steps it delivers.
- **Always real:** git, SQLite, HTTP, the hub process, the `mg` process. Time is real too; tests set debounce, poll and timeout values very small instead of faking clocks.
- **Faked, because we don't own them:** GitHub (one stateful fake server keeping PRs and comments in memory), Tailscale's local API, and judge model APIs.
- **Pure `core` logic** (comment re-mapping, interdiffs, rule matching, ordering) gets table and golden tests. Each suite keeps its cases in one data file, not one snapshot file per case.
- **UI tests** use Playwright against the same harness, with state created through `mg` commands.
- **Benchmarks** come with every speed-sensitive path a PR adds (tech-stack §9).
- **Size:** a PR holds one change, however many files it touches; work ships as a stack only when its parts are better reviewed and merged apart (`ship` skill).
- **The skill grows with the CLI.** PR 8 writes it for the commands that exist; each later PR adds its own commands to it, and the PR 8 test keeps the two in sync.

## Working on a step

- **Each step gets a brief** at `docs/roadmap/<n>-<slug>.md` (say, `docs/roadmap/3-submit.md`). It is written in the session where the operator settles the step's decisions, before any code, and ships with the step's first PR.
- **A brief fits on about one screen** and points at sections of [design.md](design.md), [user-flows.html](user-flows.html) and [tech-stack.md](tech-stack.md) by heading instead of copying them. Its sections:
  - **Scope:** what ships, and which steps of which flows.
  - **Decisions:** numbered, the ones settled with the operator first, then the small ones the agent took.
  - **Where it lives:** the crates and modules touched, new ports, migrations.
  - **Stack:** one line per PR, with its branch and status (planned, in review, merged).
  - **Open:** deferred work and doc drift to fix.
- **Implementation starts from the brief,** one session per PR of the stack. Each session updates the Stack and Open lines as it ships.
- **The step's last PR deletes the brief** along with its row below. Decisions that still matter move into design.md or tech-stack.md first.

## PRs

| # | PR | Delivers | Flows | Faked |
|---|---|---|---|---|
| 9 | GitHub backend | Push the branch, open and update the PR, squash-merge through the API as the local backend squashes, switch a project's backend (listing open items in a `needs_decision`, as a target change does) | F1, F4, F8, F15 | GitHub |
| 10 | External review | Polling PR comments, debounce (of wakes too), `external.comments`, agent verdicts and the operator's override in the UI | F5 | GitHub |
| 11 | File triage | `rules.toml`, guards, review/trusted tiers, `--guide` ordering, `mg rules propose/list`, approving looser rules in the UI | F11, F12 | — |
| 12 | Re-review | Interdiffs over a moved base, viewed marks across revisions, re-mapping drafts and comments onto newer revisions, the new-revision banner | F7, F13 | — |
| 13 | Stacked items | `mg submit --on`, own diff against the parent, `parent.updated` and `parent.abandoned`, landing bottom first, PR retargeting, stacks in the inbox and review header | F16 | GitHub |
| 14 | Prioritization | `--priority/--note`, `mg prioritize`, inbox ordering, early conflict detection, `mg events --follow` | F9, F10 | — |
| 15 | Highlighting | tree-sitter in WASM, language packs, `mg lang`, highlighted diffs | — | — (packs built in CI, installed with `--from`) |
| 16 | Demo attachments | `--attach`, `mg attach`, demo panel | F4, F6 | — |
| 17 | Judge | Judge port and providers (`command` and `anthropic` first), structural heuristics | F12 | model APIs |
| 18 | Hooks and distribution | `on_event` hooks, full `mg doctor`, cargo-dist releases, launchd/systemd units; drop `## Stage` from AGENTS.md | F1 | — |
| 19 | Multi-machine | Listening on the tailnet, tokens, Tailscale `whois`, `mg connect`, `mg hub allow`, refusing `--wake` from remote workers | F2 | Tailscale |

## Why this order

- **The harness comes first**, because every later PR is tested through it.
- **The whole loop runs on the `local` backend before GitHub** (PRs 1–8), so it is proven against real git with nothing faked.
- **GitHub comes right after the skill**, so mergegate's own work keeps landing as GitHub PRs, cloud bots keep reviewing, and the switch happens early.
- **Then the design's pain points:** triage for the flat file list, re-review for cheap revision cycles, stacks for oversized work, prioritization for many open items.
- **Polish and reach last.** PRs 15–19 stand alone and can be reordered; multi-machine waits until agents run on more than one machine.
- **Stacked items need re-review** (PR 12): keeping viewed marks and approvals across a child's rebases relies on its interdiffs and comment re-mapping. Until PR 13 lands, agents submit the parts of a split one after another.

## Switching mergegate's own reviews to mergegate

- **After PR 9:** agents submit through `mg` and the hub opens the GitHub PRs. Change `AGENTS.md` so agents ship with `mg submit` instead of the `ship` skill. If the hub breaks, the PRs are still on GitHub, so a fix can be reviewed there.
- **After PR 10:** bot and teammate comments show in mergegate, and the GitHub web UI is no longer needed.
- **From PR 9 on, the hub is in daily use:** run the latest release as the hub and dev builds next to it, as `.agents/prompts/install.md` sets up. Migrations only move forward, a migration is never edited once merged (until releases exist, `main` is what runs), and each one is tested by upgrading databases from every earlier schema version. Before PR 9 the database can be reset freely.
