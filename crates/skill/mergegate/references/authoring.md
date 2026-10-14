# Submitting and answering reviews

## Submitting

`mg submit` runs in your clone, on your branch, with your work committed. It pushes the branch to the hub and opens an item for it, or adds a revision when the branch already has an open item. It refuses a detached HEAD and the project's target branch.

- `--title` names the item; it defaults to the first commit's subject. Make it a summary the operator can scan in a queue.
- A submit at the same commit as the last revision adds no revision, but still records the worktree and any `--wake`.
- `--wake CMD` leaves a command for the hub to run when the item has events from the operator you haven't read: see [waiting.md](waiting.md).

The operator reviews the newest revision. Each revision is the whole branch against the target, so amend, rebase or add commits as you like before submitting again.

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

## The queue

`mg inbox` shows every open item: those waiting for the operator, those with their agents, and those merging.

## Giving up

`mg abandon app#3 --reason "Why"` closes an item without merging it. Do it when the operator asks, or when the work turns out not to be needed, and say why.
