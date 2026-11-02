---
name: ship
description: Commit scoped changes, review and fix them, submit them for the operator's review through mergegate (or a sequence of items when the work holds parts better reviewed apart), and follow the review until the work merges, when the user asks to ship, submit, or open a PR.
---

# Ship

Deliver a reviewed mergegate item for the requested changes. An optional user-provided title overrides the generated title. Reuse the branch's open item: submitting again from the same branch adds a revision.

This repository is reviewed through mergegate, so follow the `mergegate` skill for submitting, waiting and replying. Never `git push` or `gh pr create`: the hub pushes the branch to GitHub and opens its pull request itself. If `mg` can't reach the hub, stop and tell the operator.

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
- When `mg submit --help` lists `--on`, submit every part, each `--on` the item below it, as the mergegate skill's "Stacking work" describes: each is reviewed as its own diff and lands right after the one below, and `parent.updated` gives the rebase to run when a lower part moves.
- An older hub reviews every item as its whole branch against the target and squash-merges it, so an item above an unmerged one would show the lower part's diff again. Submit only the lowest unmerged part, then:
  - To change a lower part, commit to its branch, then rebase each part above onto the one below it, bottom up. The parts above aren't submitted yet, so rewriting them is safe.
  - Once a part merges, the squash commit doesn't match its commits, so a plain rebase would replay them. Before deleting its branch, move the next part onto the target with `git rebase --onto mergegate/<target> <merged part's branch> <next part's branch>`, then submit it.
- If shipping ends before every part is submitted, list the remaining branches, bottom first, in the report.

## Review

Use one independent read-only review subagent when available, without inherited conversation history. Write the review target to files first in a task-specific temporary directory: `git diff <merge-base>` to `diff.patch` (run `git add -N` on new files first when uncommitted changes will ship) and `git diff --name-only <merge-base>` to `files.txt`. Give the reviewer the exact checkout, base, and those absolute paths. Ask it to read repository guidance, to use plain single commands rather than loops or pipelines around `git`, to quote globs (`--glob='*.rs'`) because commands run in zsh where an unmatched glob fails, to search inside the checkout rather than from `/` or `~`, and to report concrete bugs or regressions with paths, lines, impact, and evidence, followed by one line on any tooling friction it hit.

Make finding introduced bugs and regressions the reviewer's primary objective. Give it the intended behavior and constraints, but not the implementer's conclusions about correctness or earlier review findings. Treat the changed-path list as an entry point, not a boundary: inspect relevant unchanged callers, consumers, dependencies, and tests, and compare with the base behavior. Passing checks are evidence of what was exercised, not proof that other affected flows work.

For each affected workflow, trace the trigger through state changes and side effects to the observable result. Where relevant, check failure and retry, cancellation and teardown, and work that completes after its owner or user session changes. Establish which owner orders operations and prevents obsolete work from mutating current state or recreating deleted data. For tooling that maps changed files, check deletions and renames against both revisions. Select these checks from the actual change; do not invent unrelated edge cases.

Identify the behavioral invariants implied by the requirements, policies, and callers, then check that they still hold after the first successful operation. Follow each path that exposes or changes the same state: initial load, reuse, live updates, refresh success or failure, and recovery. A validity check at creation or subscription does not establish validity for later reads or emissions: time, identity, permissions, or dependencies may have changed. Trace retained values through the consumer's rendering or side effects as well as the producer's checks. Where a lifetime boundary matters, use a concrete before/after sequence or controlled-clock probe; do not substitute an initialization-only test for that boundary.

Require each finding to identify a reachable trigger, the faulty code path, the user or operational impact, and evidence that the change introduces it. A deterministic reproduction or focused test is useful when practical; a concrete execution trace grounded in the code is also evidence. Do not dismiss an asynchronous ordering defect merely because it involves timing, and do not report one without a reachable sequence. After the correctness pass, check established architecture, unnecessary new machinery, and tests that cannot fail when the behavior breaks. Keep maintenance findings distinct from functional defects. Ask the reviewer to finish with the main flows inspected and any coverage limitations. If delegation is unavailable, review the diff directly and disclose that limitation.

Evaluate findings against the code before fixing. For an unsupported finding, identify the guard, invariant, or unreachable precondition that rules it out; passing tests alone do not rebut it. Fix supported issues within scope and rerun checks affected by those fixes. Re-review only where a fix introduces new uncertainty, refreshing the review artifacts so that review and validation cover the revision being published.

## Commit and submit

Commit messages and guides follow the authorship rule in `AGENTS.md`: the guide's intent becomes the body of the commit that lands, and on GitHub the whole guide is the pull request's description.

1. Commit the scoped changes with a concise explanation of the problem and result.
2. Write the guide to a temporary file, as the `mergegate` skill describes: the problem/result and actual validation, then a `## Files` list in reading order. On a later revision, rewrite it for the whole change as it now stands.
3. `mg submit --title "<title>" --guide <file>`. When it exits 2 with `needs_decision`, put its question and options to the operator and run it again with the flag they choose; don't choose for them.
4. Read its output: the item ID, revision, branch and target, and the pull request's link on GitHub projects. Fix the guide and submit again for any `warning:` it prints.

Remove the task-created guide file once the submit is verified.

## Follow the review

Shipping goes on until the work merges; stop once the item is submitted only when the user asked for just that. Start waiting for the item's events as the `mergegate` skill's waiting reference describes, then report the item ID, pull request link and validation, and work through the skill's loop as events arrive: answer every comment, resubmit, and wait again.

- On `item.merged` with parts still to submit, move the next part onto the target as [Size](#size) describes before the default cleanup deletes the merged branch, submit it, and follow its review the same way.
- On `item.abandoned`, a pull request closed unmerged on GitHub included, stop, and ask the operator before deleting branches or submitting a part built on it.

When the loop ends, report how: merged, or abandoned, and the parts still to submit, bottom first.
