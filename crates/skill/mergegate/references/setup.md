# Setting up mergegate

The operator asks you to set mergegate up; you run every command. Anything that needs a choice comes back as `needs_decision` (exit 2): ask the operator, then rerun with their answer. Setting a repo up takes `mg project add` and `mg init` (below); `mg skill setup` first, if `mg doctor` says an agent isn't set up, lets every agent's session on the machine load this skill ("This skill", below).

## The hub

`mg hub start` starts the hub in the background, or does nothing if it is running. It listens on 127.0.0.1 port 7800 unless given `--port N` (0 picks a free port); `--foreground` keeps it in the terminal. `mg hub status` says whether it is running and where, and `mg hub stop` stops it.

With the default data directory, `mg hub start` runs the hub as a user service (launchd on macOS, systemd on Linux), so it starts again at login and after a crash; `mg hub stop` removes the service. If the start prints a warning that the hub runs without a service, tell the operator: the hub works, but won't come back after a reboot. After `mg` is upgraded, the first command that goes through the hub restarts it on the new `mg`, and its service with it; `mg hub stop && mg hub start` does the same by hand.

`mg hub set attachment-ttl <duration>` sets how long a merged or abandoned item keeps its screenshots and recordings, as in `2d` (the default), `12h`, `30m`, or `0s` to drop them when it closes.

The hub keeps its data in `~/.mergegate`, or in `MERGEGATE_HOME` when that is set to an absolute path. `mg` finds the hub through the same directory, so a command run with another `MERGEGATE_HOME` talks to another hub.

## Another machine

Agents on other machines reach the hub over Tailscale, which must run on both machines, logged in to the same tailnet.

On the hub's machine, `mg hub set listen tailnet` makes the hub listen on the tailnet too, and prints its tailnet URL; `mg hub set listen local` goes back to loopback only. If Tailscale isn't running yet, the hub keeps trying, and `mg doctor` says why it isn't listening. On macOS it also checks that the application firewall lets the tailnet reach the hub's `mg`: the firewall keeps a rule per binary, so an upgraded `mg` without a Developer ID signature needs a rule of its own, and `mg doctor` prints the commands that add it. The same warning goes to the hub's log when it starts listening on the tailnet, and to stderr when a command restarts the hub onto an upgraded `mg`; the commands need `sudo`, so put them to the operator. A connected machine that can't reach the hub sees the same timeouts as a tailnet that's down: check Tailscale first, then `mg doctor` on the hub's machine.

On the other machine, `mg connect HUB` points `mg` at that hub, as in `mg connect mac-mini` (port 7800 unless given as `mac-mini:7801`); every command but `mg hub start`, `stop` and `status` then goes to it, and `mg init` points clones at it. `mg connect --local` goes back to the hub on this machine. `mg connect` also installs `mg wakes run` as a service, which runs the wake commands agents on this machine leave ([waiting.md](waiting.md)), and `mg connect --local` removes it; with `MERGEGATE_HOME` set it installs none, and `mg wakes run` has to run some other way. Register projects and install language packs on the hub's machine: a connected `mg` can't hand the hub a path on this one.

The hub lets in the devices of whoever runs Tailscale on its machine. Another person's or a tagged device is refused until the operator allows it: the refusal names the device and the command to run on the hub. `mg hub devices` lists the tailnet's devices and whether each may connect; `mg hub allow <device>` lets one in and `mg hub deny <device>` shuts one out, even the operator's own. Only run these when the operator tells you to.

## Projects

Each repo under review is a project. In the repo, `mg project add` registers it. Its name, used in item names, defaults to the repo's folder name (`--name NAME` picks another), and its target branch defaults to `main`, `master` or `trunk` (`--target BRANCH` picks another). Ask the operator which target to use when the repo has none of those.

A project's backend decides where approved work lands. `local` squash-merges into the target branch of the registered repo. `github` pushes each item's branch to GitHub, opens its pull request and squash-merges it there; the hub signs in as whoever the operator's `gh` CLI is logged in as, so if a command says it can't sign in, ask the operator to run `gh auth login`. A repo with a GitHub remote asks which backend to use; `--backend local|github` answers up front, and `--github OWNER/REPO` names the repository when its remotes don't settle it.

`mg project show` prints the project at the current directory, or every project. `mg project set <project> <key> <value>` changes `backend`, `target`, `upstream` or `github` (as `OWNER/REPO`) later; when open items would keep the old setting it returns `needs_decision`, and `--confirm` applies it anyway.

## Highlighting

Diffs are highlighted by language packs installed on the hub; without one a file shows as plain text, and its header names the pack that would highlight it. Each mg release comes with packs that it downloads by name. `mg project add` asks to install the ones the repo's files use (`--languages LIST|none` answers up front). For a project added without them, `mg lang add --detect [project]` installs them; without a project it uses the one at the current directory.

`mg lang list --available` prints the released packs, `mg lang add <name>...` installs them, and `mg lang list` prints what is installed and which files each pack highlights. `mg lang add --from PATH` installs a pack archive (`.tar.gz`), or every archive in the folder `PATH`. After mg is upgraded, the hub downloads again the packs built for the old one when it starts; if that fails, `mg doctor` says so, and `mg lang update` tries again. `mg lang remove <name>` removes a pack. Two packs can't claim the same grammar: remove the installed one first.

## The judge

A judge scores how much each file of a submitted revision needs the operator, ranking the files to review, and may pull a file a rule trusted back into review. It is off until the operator picks one, and only they decide which, since it sends their code to a model. `mg judge set jev` uses Jev, which reads its key from `TYPESAFE_API_KEY` in the hub's environment (`--model NAME` pins another version); a file whose diff names a password, passphrase, secret, credential, token, bearer, API, private or access key, or DSN, alone or ending a word like `PGPASSWORD`, is never sent to it and goes unjudged. `mg judge set command --command 'CMD'` runs the operator's own command in the project's repo, reading the change as JSON on stdin and printing `{"files": [{"path", "importance", "reason", "promote"}]}`; importance is 1 to 100, promote a trusted file's 0 to 1 confidence that it needs a look. `--threshold N` sets the confidence from which it promotes (default 0.8), `--project NAME` the project, and `mg judge set none` turns it off.

By default the guide's list leads the order of the files to review, and the judge only puts the files it scores 80 or more ahead of it. When the operator would rather the judge decide, `--order judge` lets its scores order every file, the guide only breaking ties; `--order guide` goes back. Changing the order or the threshold judges nothing again.

`mg judge show` prints the project's judge, whether it can run, and the revisions it failed on; setting the judge again retries them.

## Clones

Every clone or worktree you work in needs `mg init` once. It adds a `mergegate` remote pointing at the project: the one whose repo the clone is, or else the one a remote of the clone points at, by its GitHub repository, its URL on the hub, or (on the hub's machine) its repo's path. `--project NAME` picks the project when none of those settle it.

On another machine, clone the project from GitHub, or from the hub when it has no GitHub repository, naming that remote `mergegate` so that only `mg submit` pushes to it: `git clone --origin mergegate http://mac-mini:7800/git/<project>`.

## Checking

`mg doctor` checks the hub and its service, git, every project and the language packs its files use, this skill in the agents found, which `mg` is first on `PATH`, and the `mergegate` remote of the clone you run it in, and prints the fix for anything wrong. Run it after setup and whenever a command fails in a way you don't understand.

If `mg` answers with a usage line offering the options `-nR`, or opens an editor, the `mg` on `PATH` is `/usr/bin/mg`, a text editor macOS ships and some Linux distributions package. mergegate's is also installed as `mergegate`: run `mergegate doctor`, and ask the operator to run the fix its `path` check prints, which adds an `export PATH=...` to the profile their login shell reads (`~/.zprofile` for zsh), then to open a new login shell, or log in again where terminals start shells that aren't. Until they have, run `mergegate` wherever this skill says `mg`.

## Merged branches

While setting a repo up, ask the operator what to do with an item's branch after it merges: mergegate squash-merges, so git still sees the branch as unmerged. The default is `git fetch --prune mergegate`, `git switch --detach mergegate/<target>`, then `git branch -D <branch>`, which leaves the clone on the updated target. Record what you agree where you keep your standing instructions.

## This skill

`mg skill setup` sets up the coding agents on this machine: Claude Code, Codex and OpenCode, each found by the folder it keeps its settings in (`~/.claude`, `~/.codex`, `~/.config/opencode`), or only those named with `--agent`. Into each it writes this skill's stub where the agent loads skills, `~/.claude/skills` for Claude Code and `~/.agents/skills` for Codex and OpenCode. The stub sends you to `mg skill show`, which prints the guide of the installed `mg`, so upgrading `mg` updates what you read. Into Claude Code and Codex it also adds a session hook, which runs `mergegate skill session-start` as each session starts, and again after a clear or a compaction: in a clone with a `mergegate` remote it names the project, the item open on the branch, and how to wait for a review in that agent; elsewhere it says nothing. Codex runs a new hook only once it is trusted: ask the operator to trust it with `/hooks` in Codex.

Setup leaves the rest of each agent's settings as it was. It replaces what an older `mg` installed, as a copy of this skill linked into `~/.codex/skills`, and leaves a skill named mergegate that `mg` didn't write, as a conflict that `--force` replaces. `--project` sets up the current repository instead of the operator's account, and `--dry-run` only says what would change. `mg skill status` reports each agent, and `mg skill remove` removes only what an `mg` installed there, a copy of the whole skill included, and only the link where that copy was linked. Once `mg` is upgraded, the hub brings the stubs and hooks setup installed up to date as it restarts on the new `mg`, so nothing needs running again; it leaves a copy of the whole skill, which the operator may have edited, for `mg skill setup` to replace. `mg doctor` warns of an agent found but not set up, `mg connect` sets up the agents of the machine it connects, and the review UI offers the operator to set up the agents of the hub's machine.

`mg skill show NAME` prints one of the guide's references: setup, authoring, waiting or operator. For an agent setup doesn't know, `mg skill install` writes the whole skill to `$MERGEGATE_HOME/skill/mergegate` (or `--dir PATH` to `PATH/mergegate`) and prints where: copy or link the folder to where that agent loads skills from, and run it again after `mg` is upgraded.
