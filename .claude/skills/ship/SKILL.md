---
name: ship
description: Commit scoped changes, review and fix them, and submit them for the operator's review through mergegate (or a sequence of items when the work holds parts better reviewed apart) when the user asks to ship, submit, or open a PR.
---

# Ship

Deliver a reviewed mergegate item for the requested changes. An optional user-provided title overrides the generated title. Reuse the branch's open item: submitting again from the same branch adds a revision.

This repository is reviewed through mergegate, so follow the `mergegate` skill for submitting, waiting and replying. Never `git push` or `gh pr create`: the hub pushes the branch to GitHub and opens its pull request itself.

## Prepare and verify

Inspect status, staged/unstaged diffs, and recent commits. The base is the project's target as the hub has it: `git fetch mergegate`, then compare against `mergegate/<target>` (`mg project show` names the target). Work on a branch of your own, never the target; start a new one from `mergegate/<target>`.

Stage only task-owned changes. Preserve user work and keep credentials and unrelated generated artifacts out of the commit.

Run the relevant checks (`.claude/scripts/check.sh`) if they have not already passed for these changes. Do not rebuild solely because shipping began.

## Size

Ship one item per change, however many files it touches. The repository favors many small files, so file and line counts say little about how hard a change is to review, and are no reason to split.

Split the work into parts only when it holds parts the operator would rather review and merge apart: each part makes sense alone and could land without the ones above it (say, a schema change several features build on), or the operator asked for parts. When parts are warranted:

- Split along the work's own seams (layer, crate, slice), bottom-up, so each part builds only on the parts below it.
- Each part stands on its own: it builds, its checks pass, and it carries the tests for the code it adds. Do not split one change across parts.
- Name the branches `<branch>-1-<slug>`, `<branch>-2-<slug>`, and so on, each starting from the one below it.
- mergegate reviews every item as its whole branch against the target and squash-merges it, so an item above an unmerged one would show the lower part's diff again. Submit only the lowest unmerged part.
- To change a lower part, commit to its branch, then rebase each part above onto the one below it, bottom up. The parts above aren't submitted yet, so rewriting them is safe.
- Once a part merges, the squash commit doesn't match its commits, so a plain rebase would replay them. Before deleting its branch, move the next part onto the target with `git rebase --onto mergegate/<target> <merged part's branch> <next part's branch>`, then submit it.
- If shipping ends before every part is submitted, list the remaining branches, bottom first, in the report.

## Review

Use one independent read-only review subagent when available, without inherited conversation history. Give it the exact checkout, base, and review target, including uncommitted changes if that is what will be shipped. Ask it to read repository guidance and report concrete bugs or regressions with paths, lines, impact, and evidence.

The review should check affected behavior, error boundaries, established architecture, and unnecessary new machinery. Focus on realistic callers and lifecycle paths, not hypothetical races or stylistic preferences. If delegation is unavailable, review the diff directly and disclose that limitation.

Evaluate findings against the code before fixing. Fix supported issues within scope and rerun checks affected by those fixes. Re-review only where a fix introduces new uncertainty.

## Commit and submit

Commit messages and guides follow the authorship rule in `AGENTS.md`: the guide's intent becomes the body of the commit that lands, and on GitHub the whole guide is the pull request's description.

1. Commit the scoped changes with a concise explanation of the problem and result.
2. Write the guide to a temporary file, as the `mergegate` skill describes: the problem/result and actual validation, then a `## Files` list in reading order. On a later revision, rewrite it for the whole change as it now stands.
3. `mg submit --title "<title>" --guide <file>`. When it exits 2 with `needs_decision`, put its question and options to the operator and run it again with the flag they choose; don't choose for them.
4. Read its output: the item ID, revision, branch and target, and the pull request's link on GitHub projects. Fix the guide and submit again for any `warning:` it prints.

Remove the task-created guide file once the submit is verified.

Report the item ID and pull request link (and the parts still to submit, bottom first) and validation. Shipping ends once the item is submitted unless the user also asks to wait for the review, address feedback, or see it merged; then continue with the `mergegate` skill's loop.
