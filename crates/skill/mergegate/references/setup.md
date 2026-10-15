# Setting up mergegate

The operator asks you to set mergegate up; you run every command. Anything that needs a choice comes back as `needs_decision` (exit 2): ask the operator, then rerun with their answer.

## The hub

`mg hub start` starts the hub in the background, or does nothing if it is running. It listens on 127.0.0.1 port 7800 unless given `--port N` (0 picks a free port); `--foreground` keeps it in the terminal. `mg hub status` says whether it is running and where, and `mg hub stop` stops it.

With the default data directory, `mg hub start` runs the hub as a user service (launchd on macOS, systemd on Linux), so it starts again at login and after a crash; `mg hub stop` removes the service. If the start prints a warning that the hub runs without a service, tell the operator: the hub works, but won't come back after a reboot. After upgrading `mg`, run `mg hub stop && mg hub start` so the service runs the new one.

`mg hub set attachment-ttl <duration>` sets how long a merged or abandoned item keeps its screenshots and recordings, as in `2d` (the default), `12h`, `30m`, or `0s` to drop them when it closes.

The hub keeps its data in `~/.mergegate`, or in `MERGEGATE_HOME` when that is set to an absolute path. `mg` finds the hub through the same directory, so a command run with another `MERGEGATE_HOME` talks to another hub.

## Another machine

Agents on other machines reach the hub over Tailscale, which must run on both machines, logged in to the same tailnet.

On the hub's machine, `mg hub set listen tailnet` makes the hub listen on the tailnet too, and prints its tailnet URL; `mg hub set listen local` goes back to loopback only. If Tailscale isn't running yet, the hub keeps trying, and `mg doctor` says why it isn't listening.

On the other machine, `mg connect HUB` points `mg` at that hub, as in `mg connect mac-mini` (port 7800 unless given as `mac-mini:7801`); every command but `mg hub start`, `stop` and `status` then goes to it, and `mg init` points clones at it. `mg connect --local` goes back to the hub on this machine. Register projects and install language packs on the hub's machine: a connected `mg` can't hand the hub a path on this one.

The hub lets in the devices of whoever runs Tailscale on its machine. Another person's or a tagged device is refused until the operator allows it: the refusal names the device and the command to run on the hub. `mg hub devices` lists the tailnet's devices and whether each may connect; `mg hub allow <device>` lets one in and `mg hub deny <device>` shuts one out, even the operator's own. Only run these when the operator tells you to.

## Projects

Each repo under review is a project. In the repo, `mg project add` registers it. Its name, used in item IDs, defaults to the repo's folder name (`--name NAME` picks another), and its target branch defaults to `main`, `master` or `trunk` (`--target BRANCH` picks another). Ask the operator which target to use when the repo has none of those.

A project's backend decides where approved work lands. `local` squash-merges into the target branch of the registered repo. `github` pushes each item's branch to GitHub, opens its pull request and squash-merges it there; the hub signs in as whoever the operator's `gh` CLI is logged in as, so if a command says it can't sign in, ask the operator to run `gh auth login`. A repo with a GitHub remote asks which backend to use; `--backend local|github` answers up front, and `--github OWNER/REPO` names the repository when its remotes don't settle it.

`mg project show` prints the project at the current directory, or every project. `mg project set <project> <key> <value>` changes `backend`, `target`, `upstream` or `github` (as `OWNER/REPO`) later; when open items would keep the old setting it returns `needs_decision`, and `--confirm` applies it anyway.

## Highlighting

Diffs are highlighted by language packs installed on the hub; without one a file shows as plain text, and its header names the pack that would highlight it. Each mg release comes with packs that it downloads by name. `mg project add` asks to install the ones the repo's files use (`--languages LIST|none` answers up front). For a project added without them, `mg lang add --detect [project]` installs them; without a project it uses the one at the current directory.

`mg lang list --available` prints the released packs, `mg lang add <name>...` installs them, and `mg lang list` prints what is installed and which files each pack highlights. `mg lang add --from PATH` installs a pack archive (`.tar.gz`), or every archive in the folder `PATH`. After mg is upgraded, `mg lang update` downloads again the packs built for the old one; `mg doctor` says when that is needed. `mg lang remove <name>` removes a pack. Two packs can't claim the same grammar: remove the installed one first.

## Clones

Every clone or worktree you work in needs `mg init` once. It adds a `mergegate` remote pointing at the project; `--project NAME` picks the project when the clone isn't the registered repo.

## Checking

`mg doctor` checks the hub and its service, git, every project and the language packs its files use, this skill, and the `mergegate` remote of the clone you run it in, and prints the fix for anything wrong. Run it after setup and whenever a command fails in a way you don't understand.

## This skill

`mg skill install` writes this skill, matching the installed `mg`, to `$MERGEGATE_HOME/skill/mergegate` (or `--dir PATH` to write it to `PATH/mergegate`) and prints where. Copy or link the folder to wherever you load skills from. Rerun it after `mg` is upgraded; it replaces the folder.
