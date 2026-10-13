# Install mergegate on this machine

Set up two installations of mergegate that never share state:

- **Stable**: the latest release. It runs the hub the operator reviews with, and agents submit their work through it.
- **Dev**: built from this checkout, to try changes to mergegate itself.

## Stable

1. Find the latest release: `gh release view --repo damian-kolasinski/mergegate --json tagName --jq .tagName`. If there is none yet, use `main`.
2. Install it: `cargo install --locked --force --git https://github.com/damian-kolasinski/mergegate --tag <tag> mg` (`--branch main` when there is no release). This puts `mg` in `~/.cargo/bin`. Check that `command -v mg` resolves there.
3. Start the hub: `mg hub start`. Its data lives in `~/.mergegate`.
4. In the repo, run `mg project add`, `mg init` and `mg skill install`. When a command returns `needs_decision`, ask the operator and re-run it with their answer.
5. Run `mg doctor` and fix what it reports.

To upgrade, repeat step 2 with the new tag, then `mg hub stop` and `mg hub start`. Before migrating, the hub backs up its database as `hub.db.bak-<schema version>` in `~/.mergegate`.

Prefer fixing forward to rolling back: a rollback loses review state written since the upgrade. If you must roll back:
1. Run `mg hub stop`.
2. Install the previous tag as in step 2.
3. Delete `hub.db-wal` and `hub.db-shm`.
4. Copy the newest `hub.db.bak-*` over `hub.db`.
5. Run `mg hub start`.

## Dev

1. Build from the checkout: `cargo build`. The binary is `target/debug/mg`.
2. Point every dev command at its own data directory, as an absolute path: `export MERGEGATE_HOME="<absolute path to this checkout>/.mg-dev"`. The CLI finds the dev hub's address there.
3. Run the dev hub in the foreground on its own port: `target/debug/mg hub start --foreground --port 7801`.
4. Try changes on a scratch clone, never on this checkout: `mg init` there would replace the stable hub's `mergegate` remote.

`.mg-dev/` is ignored by git. Delete it to start from an empty hub.

## Rules

- Submit work with the stable `mg` on `PATH`. Use `target/debug/mg` only to try changes.
- Never run a dev build without `MERGEGATE_HOME`. It would migrate the stable hub's database to a schema the released binary may not read.
- Never run `mg skill install` with a dev build. It overwrites the stable skill that agents use.
- Tests need neither installation; they start their own hubs.
