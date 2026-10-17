# Submitting and answering reviews

## Submitting

`mg submit` runs in your clone, on your branch, with your work committed. It pushes the branch to the hub and opens an item for it, or adds a revision when the branch already has an open item. It refuses a detached HEAD and the project's target branch.

- `--title` names the item; it defaults to the first commit's subject. Make it a summary the operator can scan in a queue.
- A submit at the same commit as the last revision adds no revision, but still records the worktree, any `--guide` and any `--wake`.
- `--guide FILE` (or `-` for stdin) sends a guide for the review: see below.
- `--priority p0|p1|p2|p3` and `--note "Why"` pass on how urgent the work is, when your brief says. Only the submit that opens the item sets them; later ones warn and keep what the operator set, which `mg prioritize` changes (see [operator.md](operator.md)).
- `--wake CMD` leaves a command for the hub to run when the item has events from the operator you haven't read: see [waiting.md](waiting.md).
- `--on ITEM` stacks the item on another: see "Stacking work" below.

The operator reviews the newest revision. Each revision is the whole branch against the target (a stacked item's, against its parent: see below), so amend, rebase or add commits as you like before submitting again.

The hub test-merges open items as their target moves. When yours stops merging cleanly, a `merge.conflict` can come before the operator reviews it, once per revision: rebase as it says and submit, and the item is back in the operator's queue.

## Stacking work

Work too big for one review goes up as a stack: a branch per part, each built on the one below, each its own item. Submit the bottom part as usual, then branch from it, commit the next part and `mg submit --on app#3` (or `--on 3`). The operator reviews each item as only its own commits, against the revision of the item below that it's built on, and an approved item lands right after the one below it.

- The parent sticks across submits; `--on` changes it, and `--on none` takes the item off the stack.
- Stacks are linear. A second `--on` to an item that already has one stacked on it is refused, naming that item: stack on it instead.
- The branch must contain a revision of its parent, so build it on the parent's branch or rebase onto it; a closed item can't be stacked on.
- When the parent gets a new revision, you get `parent.updated` with the `git rebase --onto` that moves only your own commits onto it. When the parent merges and yours doesn't apply cleanly after it, `parent.updated` says to move them onto the target; an approved item waits for that revision and merges on the same approval when its diff is unchanged.
- When the parent is abandoned, `parent.abandoned` lists the commands that move your commits onto the target. Once the parent is closed, your next revision is reviewed against the target.
- On GitHub, a stacked item's pull request is based on its parent's branch until the parent lands.

## The guide

A guide tells the operator what the change is for and where to start. It is Markdown: a few lines on the intent, then a `## Files` list with one bullet per file worth reading, in the order to read them:

```markdown
Splits parsing out of main, so the CLI can reuse it.

## Files

- `src/parser.rs` — the new parser, start here
- `src/main.rs` — now only calls it
```

The operator sees the whole guide as plain text above the diff, and the files to review in its order, each with its reason; files it leaves out follow in path order. A new revision submitted without `--guide` keeps the last guide, so send a new one when the change moves; `--guide /dev/null` removes it. `mg submit` prints a `warning:` line (`warnings` in `--json`) for each thing it can't place, such as a file the revision doesn't change or one the operator's rules trust: fix the guide and submit it again.

The guide outlives the review. Its intent, the text before `## Files`, becomes the body of the commit that lands when the item merges, and on GitHub the whole guide is the pull request's description. So write it about the change as a whole, for someone reading the history later: no revision numbers (`r2`), comment IDs (`c_17`) or "as asked". On a later revision, rewrite the guide for the whole change as it now stands. What the revision changed belongs in your replies to the comments it answers (see below), which the operator reads on each comment's thread next to the diff from the last revision. `mg submit` warns when the intent names one of the item's comments or revisions.

## Showing the change

When the change has something to see, such as a screen, a flow or output, record it and attach the files, so the operator watches it before reading the diff. `mg submit --attach FILE` attaches a file to the revision it submits; repeat `--attach` for more. `mg attach <item> FILE...` adds files to the item's newest revision later. Each file must be a PNG, JPEG, GIF or WebP image, or an MP4 or WebM video, of at most 100 MB; `mg submit` checks them before it pushes, so a wrong file stops it with nothing submitted. If a submit pushed but an upload failed, it says so: rerun the upload with `mg attach`.

The review screen shows the newest revision's attachments, or the newest earlier revision's while it has none, so a small fix needn't be recorded again. They stay in mergegate, even for GitHub projects.

## What the operator reads

The operator's rules sort each file of a revision into review or trusted: trusted files, such as lockfiles or generated code, are folded away and the rest are read. `mg rules list` shows the project's rules, first match wins. The operator writes them; don't edit the rules file. When the operator asks you for a rule, propose it: see [operator.md](operator.md). A change to a `.gitattributes` is always reviewed, as it can mark files generated.

## Reading a review

A `review.submitted` event carries the verdict (`approved`, `changes_requested` or `comment`), the operator's summary, and the comments. Each comment has an ID (`c_17`), the file and lines it is on, the lines it quotes, and its body. `replies` holds the operator's answers to your replies on earlier comments.

## Replying

Answer every comment, once you know what you're doing about it:

- `mg reply c_17 --status addressed` when the change is in your next revision. Add `--body` if the fix isn't obvious from the diff.
- `mg reply c_17 --status rejected --body "Why not"` when you won't change it.
- `mg reply c_17 --status question --body "What you need to know"` when you can't act until the operator answers.

The latest reply on a comment is the one that counts, so reply again when you change your mind. Replies stay in mergegate; nothing is posted to GitHub. Then `mg submit` the revision with the changes, and wait again.

## Comments from GitHub

On an item with a GitHub pull request, the hub reads the pull request's comments for you: inline ones, conversation comments, and review summaries, from people and bots, not the operator's own. It holds them until the reviewers are done, so you get one `external.comments` event instead of one per comment: until the project's bots (`mg project set app bots coderabbitai,copilot`) have all commented since your latest submit, and then a minute goes by with no new comment, or at most 20 minutes.

- Each comment has an ID like the operator's (`c_23`) and names its author. One on lines no revision's diff shows, or on no file, is about the whole item.
- Answer them with `mg reply` as you would the operator's. Your replies stay in mergegate; nothing is posted to GitHub, and the operator sees them.
- A comment edited on GitHub comes back in a later batch marked edited.
- The operator can agree with a rejection, or override it with `comment.override`: then make the change.
- `mg feedback app#3` hands over the held comments at once along with anything else new. `mg sync app#3` reads the pull request now and hands over everything held; use it when you know a comment is there that never arrived.

The pull request can also end on GitHub, without the operator. Merged there, the item merges with it: `item.merged` says it was merged on GitHub and as which commit. Closed there unmerged, the item is abandoned: `item.abandoned` says GitHub abandoned it. Either way the item is closed; there's nothing left to submit.

## The queue

`mg inbox` shows every open item: those waiting for the operator, those with their agents, and those merging, each with the files the operator has to read when rules trust some, as in `4 of 6 files to review`. Each group is ordered by priority (`p0` first, then none), then oldest first, with each item's note under it.

## Giving up

`mg abandon app#3 --reason "Why"` closes an item without merging it. Do it when the operator asks, or when the work turns out not to be needed, and say why.
