# Setting up mergegate

The operator asks you to set mergegate up; you run every command. Anything that needs a choice comes back as `needs_decision` (exit 2): ask the operator, then rerun with their answer.

## The hub

`mg hub start` starts the hub in the background, or does nothing if it is running. It listens on 127.0.0.1 port 7800 unless given `--port N` (0 picks a free port); `--foreground` keeps it in the terminal. `mg hub status` says whether it is running and where, and `mg hub stop` stops it.

The hub keeps its data in `~/.mergegate`, or in `MERGEGATE_HOME` when that is set to an absolute path. `mg` finds the hub through the same directory, so a command run with another `MERGEGATE_HOME` talks to another hub.

## Projects

Each repo under review is a project. In the repo, `mg project add` registers it. Its name, used in item IDs, defaults to the repo's folder name (`--name NAME` picks another), and its target branch defaults to `main`, `master` or `trunk` (`--target BRANCH` picks another). Ask the operator which target to use when the repo has none of those.

`mg project show` prints the project at the current directory, or every project. `mg project set <project> <key> <value>` changes `backend`, `target` or `upstream` later; when open items would keep the old setting it returns `needs_decision`, and `--confirm` applies it anyway.

## Clones

Every clone or worktree you work in needs `mg init` once. It adds a `mergegate` remote pointing at the project; `--project NAME` picks the project when the clone isn't the registered repo.

## Checking

`mg doctor` checks the hub, git and every project, and prints the fix for anything wrong. Run it after setup and whenever a command fails in a way you don't understand.

## This skill

`mg skill install` writes this skill, matching the installed `mg`, to `$MERGEGATE_HOME/skill/mergegate` (or `--dir PATH` to write it to `PATH/mergegate`) and prints where. Copy or link the folder to wherever you load skills from. Rerun it after `mg` is upgraded; it replaces the folder.
