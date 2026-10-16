# Install mergegate on this machine

Set up two installations of mergegate that never share state:

- **Stable**: the latest release. It runs the hub the operator reviews with, and agents submit their work through it.
- **Dev**: built from this checkout, to try changes to mergegate itself.

## Stable

1. Find the latest release: `gh release view --repo damian-kolasinski/mergegate --json tagName --jq .tagName`.
2. Install it with mise: `mise use -g packslip:damian-kolasinski/mergegate`. mise checks the release's packslip signature before it installs `mg`. If `mise backends ls` doesn't list `packslip`, run `mise self-update` first. Check that `command -v mg` resolves into mise and that no `mg` from an earlier source install shadows it; `cargo uninstall mg` removes one from `~/.cargo/bin`.
3. If there is no release yet, build from `main` instead: `MG_REQUIRE_UI=1 cargo install --locked --force --git https://github.com/damian-kolasinski/mergegate --branch main mg`. The build needs pnpm and Node 22 for the UI; `MG_REQUIRE_UI=1` makes it fail rather than install an `mg` without one. It installs into `~/.cargo/bin`, or `DIR/bin` with `--root DIR`.
4. Check that `command -v mg` prints the `mg` you installed. macOS ships an unrelated `/usr/bin/mg`, a text editor, which wins whenever the install directory isn't on `PATH` ahead of `/usr/bin`; an agent calling `mg` then gets `usage: mg [-nR] ...` or an editor. mergegate's `mg` is also installed as `mergegate`: if `command -v mg` prints another, run `mergegate doctor` and the fix its `path` check prints, which appends an `export PATH=...` to the profile the login shell reads (`~/.zprofile` for zsh, not `~/.zshenv`, which macOS's `/etc/zprofile` overrides), then open a new shell and check again.
5. Start the hub: `mg hub start`. Its data lives in `~/.mergegate`.
6. In the repo, run `mg project add`, `mg init` and `mg skill install`. When a command returns `needs_decision`, ask the operator and re-run it with their answer. `mg project add` offers to download the language packs the repo's files use; a build from `main` can't download them, so build them as in Dev below, install them with `mg lang add --from DIR`, and pass `--languages none`.
7. Link the skill to where agents load skills from, so `mg skill install` updates it in place: `ln -sfn ~/.mergegate/skill/mergegate ~/.claude/skills/mergegate`, and the same into `~/.codex/skills` for Codex.
8. Run `mg doctor` and fix what it reports.

To upgrade, run `mise up packslip:damian-kolasinski/mergegate`, then `mg hub stop`, `mg hub start` and `mg lang update`. Until the hub restarts it runs the old `mg`: `mg hub status` and `mg doctor` warn of it, and if the upgrade changed the API, every command that goes through the hub fails with `hub_incompatible`. Before migrating, the hub backs up its database as `hub.db.bak-<schema version>` in `~/.mergegate`.

Prefer fixing forward to rolling back: a rollback loses review state written since the upgrade. If you must roll back:
1. Run `mg hub stop`.
2. Install the previous version: `mise use -g packslip:damian-kolasinski/mergegate@<version>`.
3. Delete `hub.db-wal` and `hub.db-shm`.
4. Copy the newest `hub.db.bak-*` over `hub.db`.
5. Run `mg hub start`.

## Another machine

Agents on another machine of the tailnet submit to the stable hub.

1. On the hub's machine, run `mg hub set listen tailnet`.
2. On the other machine, install the same `mg` as the hub's: a connected `mg` refuses a hub with another API version. With a release, install it with mise as above. Without one, build the same commit, or copy the hub machine's binaries when both share an OS and architecture: `scp "$(command -v mg)" "$(command -v mergegate)" HOST:.local/bin/`. Check `command -v mg` there as in Stable step 4, in the login shell agents run in.
3. There, run `mg connect HUB` with the hub machine's tailnet name, then `mg skill install` and link the skill as in Stable step 7.
4. Run `mg doctor` there. Register projects only on the hub's machine; clone them there from GitHub, or from the hub as its `mergegate` remote (`git clone --origin mergegate http://HUB:7800/git/<project>`), so no `origin` pushes to the hub, and `mg init` finds the project from the clone's remote.

After upgrading the hub, install the same `mg` on each other machine again and rerun `mg skill install` there.

## Dev

1. Build from the checkout: `cargo build`. The binary is `target/debug/mg`.
2. Point every dev command at its own data directory, as an absolute path: `export MERGEGATE_HOME="<absolute path to this checkout>/.mg-dev"`. The CLI finds the dev hub's address there.
3. Run the dev hub in the foreground on its own port: `target/debug/mg hub start --foreground --port 7801`.
4. Try changes on a scratch clone, never on this checkout: `mg init` there would replace the stable hub's `mergegate` remote.
5. For UI work, run `pnpm --dir ui watch` and start the dev hub with `MG_UI_DIR="$PWD/ui/dist"`. It then serves the UI from disk, so a reload shows each rebuild without rebuilding `mg`.

A dev build can't download language packs by name. Build them with `node langs/build.mjs --out langs/dist` and install them with `target/debug/mg lang add --from langs/dist`.

`.mg-dev/` is ignored by git. Delete it to start from an empty hub.

## Rules

- Submit work with the stable `mg` on `PATH`. Use `target/debug/mg` only to try changes.
- Never run a dev build without `MERGEGATE_HOME`. It would migrate the stable hub's database to a schema the released binary may not read.
- Never run `mg skill install` with a dev build. It overwrites the stable skill that agents use.
- Tests need neither installation; they start their own hubs.
