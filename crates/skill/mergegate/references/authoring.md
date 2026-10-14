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

## The queue

`mg inbox` shows every open item: those waiting for the operator, those with their agents, and those merging.

## Giving up

`mg abandon app#3 --reason "Why"` closes an item without merging it. Do it when the operator asks, or when the work turns out not to be needed, and say why.
