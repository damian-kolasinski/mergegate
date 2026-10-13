# 3 — Submit

## Scope

Workers push to the hub and register their work as items: `mg init`, `mg submit`, revisions, `mg inbox --json`, and each revision's file stats against its merge-base with the target. `mg project set` lists open items in a `needs_decision` before changing the backend or target.

Flows: F4 steps 3–6 and 8 (no `--guide`, `--attach`, `--priority`, `--note`, GitHub or triage yet), F15's open items. Design: §4 Topology, §5 Item lifecycle, §6.0–6.2. Tech stack: §5 Git split.

## Decisions

The agent took all of these; none is settled with the operator yet. Confirm or change them in review.

1. **The hub is a smart-HTTP remote at `<hub>/git/<project>`.** axum runs `git http-backend` as a CGI with bodies streamed both ways, under the same sanitized environment as every other git call (tech-stack §5).
2. **Workers can't write the hub's pins.** `transfer.hideRefs` hides `refs/mergegate` from pushes and fetches. The target stays visible, so a push sends only the commits the hub lacks. The hub force-fetches the target from the upstream before every use (a submit, a `git fetch mergegate`), so a push to it doesn't stick.
3. **Every revision is pinned** at `refs/mergegate/items/<n>/r<rev>`, so a later force-push to the branch can't drop the commits an older revision points at.
4. **`git push` and `git fetch` pass the CSRF guard by their content type** (`application/x-git-*-request`), which a browser can't send cross-origin without a preflight. Everything else still needs `x-mergegate-client`.
5. **`mg init [--project NAME]`** adds (or re-points) the `mergegate` remote. The project defaults to the one registered for the current repo.
6. **`mg submit [--title T]`** pushes `+HEAD:refs/heads/<branch>` to the hub and then registers the revision with the head it pushed. It refuses a detached HEAD and the target branch, naming the fix.
7. **Items are numbered per project** (`app#1`) and keep the backend and target they were opened with until they close (design §6.0). One open item per project and branch.
8. **Submitting** fetches the item's target, checks the pushed branch is at the given head, and records the merge-base and `git diff --numstat -M` against it (gix and imara-diff arrive with the diff view in PR 5). A head on the target is refused as nothing to submit; the same head again returns the current revision. The default title is the first commit's subject.
9. **A new revision puts the item back in needs review**, whatever open state it was in.
10. **`mg inbox --json`** groups open items as `needs_you`, `with_agents` and `merging`, oldest first until priorities arrive (PR 14).
11. **`mg project set`** asks for `--confirm` when a backend or target change leaves open items on the old setting, listing them.
12. **The git port is split per slice** like the store: `ItemGit` sits next to `ItemStore`, and `HubGit` implements both.

## Where it lives

- `api::item`: wire types. `core::item`: IDs, states, `submit`, `inbox`, ports `ItemStore` and `ItemGit`. `core::project::set` gains the open-items check.
- `store-sqlite`: migration `0003_items.sql` (items, revisions, revision files).
- `git`: item reads and writes, and the `http-backend` command.
- `hub`: `git_http` and `items` routes, guard. `cli`: `init`, `submit`, `inbox`, `project set --confirm`.

## Stack

- `submit-1-domain`: brief, wire types, core item slice — in review
- `submit-2-adapters`: SQLite and git adapters — in review
- `submit-3-commands`: hub routes, smart-HTTP, CLI, scenario tests — planned

## Open

- Deferred to their PRs: `--base`, `--guide`, `--attach`, `--priority`/`--note`, `--on`, `--wake`.
- A target renamed to or from a nested name (`release` and `release/1.0`) can't coexist in staging while an open item keeps the old one; the fetch fails with git's error.
- Pins are never deleted yet; closing items (PR 7) should drop them.
