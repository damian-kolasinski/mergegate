# gix 0.89

The git adapter (`crates/git`) reads repositories with gix and writes through the `git` binary (`command.rs`), which runs in a sanitized environment. These calls were checked against gix 0.89.0 built with `default-features = false, features = ["sha1"]`, which covers all of them.

## Calls

| Need | Call |
|---|---|
| Find the repo containing a path, searching upwards | `gix::discover_opts(path, Default::default(), gix::open::Options::isolated())` |
| Open a repo at an exact path | `gix::open_opts(dir, gix::open::Options::isolated())` |
| The repo that owns a linked worktree | `repo.main_repo()?` (returns the repo itself when it is not a linked worktree) |
| The repo's folder | `repo.workdir()` (`None` when bare), `repo.is_bare()`, `repo.git_dir()` |
| Local branches | `repo.references()?.local_branches()?` yields `Result<Reference>`; `r.name().shorten()` gives `main` |
| The checked-out branch | `repo.head_name()?` gives `Option<FullName>`, `None` when HEAD is detached; `.shorten()` |
| Look up one ref | `repo.try_find_reference("refs/heads/main")?` gives `Option<Reference>` |

`Options::isolated()` ignores system and global config and `GIT_*` variables, so gix sees the same config as the sanitized `git` binary.

## Errors

- These calls return `gix::Result<T>`, whose error is `gix::Error` (re-exported from `gix-error`). The concrete cause stays inside it; reach it with `e.downcast_any_ref::<T>()`.
- "No repo here" from discovery is one of `gix::discover::upwards::Error::{NoGitRepository, NoGitRepositoryWithinCeiling, NoGitRepositoryWithinFs}`. Match those and report anything else, such as a corrupt `.git` or missing permissions, as a failure. `crates/git/src/inspect.rs` does this.
- Discovery must start from an existing directory: a missing path fails with an I/O error and a file path with a validation error. Check the path first, as `inspect.rs` does, starting from a file's parent.
- `gix::error::classify(&e).is_not_found()` is a coarser check. It is true for "no repo" and a missing path, but not for a file path.
