# 18a — Distribution

## Scope

mergegate installs without a Rust, Node or pnpm toolchain: tagged releases with signed binaries, language packs downloaded by name, the hub kept running by the OS, and an `mg doctor` that checks the whole setup. F1 end to end (design.md §6.0, tech-stack.md §3 Packaging and §6).

## Decisions

Settled with the operator:

1. **The repository is public,** so releases, packs and installers need no credentials.
2. **Releases are signed with [packslip](https://packslip.dev), not built with cargo-dist.** A `vX.Y.Z` tag runs `.github/workflows/release.yml`. It builds `mg-<target>.tar.gz` for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`, signs them keylessly with `jdx/packslip` (`packslip.sigstore.json`, plus GitHub build-provenance attestations), and publishes one release holding all of it. Operators install with `mise use -g packslip:damian-kolasinski/mergegate` and upgrade with `mise up`. No Homebrew tap, no curl installer of our own, no `mg self-update`.
3. **`mg hub start` installs a user service** (launchd agent on macOS, systemd `--user` unit on Linux) that starts the hub at login and restarts it after a crash; `mg hub stop` unloads and removes it. `--foreground` and a set `MERGEGATE_HOME` (dev hubs, tests) keep the detached process and never touch service files.
4. **Three PRs,** below. The operator pushes the first tag once the stack merges.

Taken by the agent:

5. **Linux binaries link glibc** and build on Ubuntu 22.04 runners, so they run on glibc 2.35 and newer. macOS builds both architectures on one arm64 runner.
6. **Each release ships the language packs built by the same run,** with `languages.json` (name, version, file, sha256). The build bakes that index into `mg` (`MG_LANGUAGES_INDEX`), so the binary pins the digests it accepts and packslip's signature covers them. The hub downloads `releases/download/v<its version>/<file>` and checks the digest; a pack ABI change ships as a new mg version. A build without the index (dev, tests) can't download by name and offers only `--from`. Tests point `MG_LANGUAGES_INDEX` and `MG_LANGUAGES_URL` at a local server instead.
7. **Detection** counts the files on the target branch per pack of the index. `mg project add` asks `--languages <list>|none` when the repo uses packs that aren't installed, defaulting to all of them, and installs them after registering; `mg lang add --detect [<project>]` installs the same set.
8. **The unit records the PATH of the shell that ran `mg hub start`,** since launchd and systemd start services with a minimal one and the hub runs `gh`, git and wake commands.
9. **`mg doctor` adds:** the hub's version against this `mg`, the service unit and the binary it runs, the current repo's `mergegate` remote, the installed skill against this binary's, packs not from this mg's index (`mg lang update`), and a project's detected languages without a pack.

## Where it lives

- `.github/workflows/release.yml`; `langs/build.mjs` writes `languages.json`.
- `crates/lang-release` (new adapter): the baked index, downloads and digest checks. `crates/hub/src/languages.rs` wires it; `core::highlight::detect`, `core::project::languages`, `Git::files`.
- `crates/cli/src/hub_cmd/service*` for the units; `crates/cli/src/doctor_cmd.rs` and `crates/core/src/doctor.rs` for the checks.
- No migration.

## Stack

1. `18a-distribution-1-releases`: release workflow, packs as release assets, install.md, this brief. Merged.
2. `18a-distribution-2-lang-download`: `mg lang add <name>`, `--available`, `--detect`, `update`, detection in `mg project add`, pack checks in `mg doctor`, the index baked into release builds. In review.
3. `18a-distribution-3-service`: service units, the rest of `mg doctor`, docs; drops `## Stage` from AGENTS.md, this brief and the roadmap row. Planned.

## Open

- After the first release: put packslip's pin (`ps1_…`) in install.md, and propose `mergegate` to the mise registry so `mise use mergegate` works.
- Verifying pack downloads against packslip's bundle directly, if packs ever ship apart from mg.
