---
name: ship
description: Commit scoped changes, review and fix them, and create or update a ready-for-review pull request (or a stack of them when the work exceeds the PR size limit) when the user asks to ship or open a PR.
---

# Ship

Deliver a reviewed PR for the requested changes. An optional user-provided title overrides the generated title. Reuse an existing PR for the branch.

## Prepare and verify

Inspect status, staged/unstaged diffs, and recent commits. Identify the intended base from the user or existing PR; otherwise use the repository's default branch. Fetch it when needed for an accurate comparison.

Stage only task-owned changes. Preserve user work and keep credentials and unrelated generated artifacts out of the commit.

Run the relevant checks if they have not already passed for these changes. Do not rebuild solely because shipping began.

## Size limit

A PR changes at most 25 files against its base. Lockfiles and files generated from committed sources (such as TypeScript types generated from `api`) do not count. After staging, check with `git diff --cached --name-only $(git merge-base origin/<base> HEAD)`, so committed, staged and new files are all counted.

If the work is over the limit, ship it as a stack of PRs instead of one:

- Split along the work's own seams (layer, crate, slice), bottom-up, so each PR builds only on the PRs below it.
- Each PR stands on its own: it builds, its checks pass, and it carries the tests for the code it adds. Do not split one change across PRs just to meet the count.
- The bottom PR targets the base branch; each PR above targets the branch of the PR below. Name the branches `<branch>-1-<slug>`, `<branch>-2-<slug>`, and so on. If the working branch already has a PR, keep it as the top of the stack: merge the branch directly below into it and retarget its PR to that branch.
- Before creating anything, look up existing PRs for every stack branch (`gh pr list --head <branch>`) and reuse them.
- Review, push and publish each PR against its own base, bottom first. Then edit every body so it starts with the stack: its position, links to the other PRs, and that PRs merge bottom first with a merge commit (not squash or rebase).
- To change a lower PR, commit to its branch, then merge each branch into the one directly above it, bottom up. Do not rebase published stack branches.
- After a lower PR merges, retarget the PR above it to the base branch. If shipping ends before that, list the pending retargets in the report.

If the work has no seam that keeps every piece coherent under the limit, ship one PR and explain why in its body.

## Review

Use one independent read-only review subagent when available, without inherited conversation history. Give it the exact checkout, base, and review target, including uncommitted changes if that is what will be shipped. Ask it to read repository guidance and report concrete bugs or regressions with paths, lines, impact, and evidence.

The review should check affected behavior, error boundaries, established architecture, and unnecessary new machinery. Focus on realistic callers and lifecycle paths, not hypothetical races or stylistic preferences. If delegation is unavailable, review the diff directly and disclose that limitation.

Evaluate findings against the code before fixing. Fix supported issues within scope and rerun checks affected by those fixes. Re-review only where a fix introduces new uncertainty.

## Commit and publish

Commit messages and PR bodies follow the authorship rule in `AGENTS.md`.

1. Commit the scoped changes with a concise explanation of the problem and result. Push the working branch (every branch of a stack); do not push directly to the base branch or force-push without task authorization.
2. Inspect the existing PR with `gh pr view --json number,url,baseRefName,body`. Distinguish no PR from authentication/network errors.
3. Write the exact PR body to a temporary file and publish with `--body-file`. Describe the problem/result and actual validation.
4. Update the existing PR or create a ready-for-review PR with the intended base and head. Follow the repository template where present. Avoid duplicate PRs after an uncertain command result: inspect remote state first.
5. Read back the published title/body and URL.

Keep local body artifacts until publication is verified; remove only task-created temporary files afterward.

Report the PR URL (every URL in a stack, bottom first) and validation. Shipping ends after publication is verified unless the user also asks to monitor CI, address subsequent feedback, or merge.
