# Git

## Split

| Operation | Implementation | Why |
|---|---|---|
| Receive pushes from workers | `git http-backend` (CGI) behind axum | gix has no server side. |
| Test-merge, merge | `git merge-tree --write-tree` (git ≥ 2.38) | Must match git's `ort` merge exactly, or conflicts are detected wrongly. |
| Push to GitHub / operator repo | `git push` (`updateInstead` for local) | gix push is incomplete; must respect git semantics. |
| Read blobs, trees, history | gix | No process spawn per call (~2–5 ms each on macOS); typed results. |
| File diffs, interdiffs | gix tree diff + imara-diff (histogram) | In-memory; yields the line mapping that comment anchoring needs. |

Everything goes through the single `GitRepo` port, so any operation can move between gix and the `git` binary without callers changing.

## Environment sanitization

Subprocess calls on the hub's own repos run with:

- `GIT_CONFIG_NOSYSTEM=1` and `GIT_CONFIG_GLOBAL=/dev/null`;
- an isolated `HOME` and `XDG_CONFIG_HOME`;
- explicit `-c` overrides for every setting that affects output;
- hooks disabled.

The operator's personal git config must never change output.

**Exception: the `local` backend's push into the operator's repo.** The receiving side runs in that repo, so its config and hooks apply, which is intended because it's the operator's repo. `-c` overrides don't reach a local `receive-pack`, so the hub passes `--receive-pack="git -c receive.denyCurrentBranch=updateInstead receive-pack"` instead of changing the operator's repo config. Nothing is ever force-updated.

## Interdiffs

An interdiff shows what changed between two revisions of an item (r1 → r2). If the target branch advanced in between, a raw r1→r2 diff includes upstream changes. The diff engine compares each revision against its own merge-base (range-diff semantics) and shows only the item's own changes.
