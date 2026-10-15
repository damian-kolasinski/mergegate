//! `mergegate`: `mg` under a name no other tool takes. macOS ships an
//! unrelated editor as `/usr/bin/mg`, which a PATH listing `/usr/bin` early
//! finds first. This runs the `mg` installed beside it instead of linking a
//! second copy of the whole CLI.

use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    // Resolved, so a link to this binary still finds the `mg` it came with.
    let mg = match std::env::current_exe().and_then(std::fs::canonicalize) {
        Ok(exe) => exe.with_file_name("mg"),
        Err(e) => {
            eprintln!("mergegate: locating mg: {e}");
            return ExitCode::FAILURE;
        }
    };
    // Replaces this process, so the caller gets mg's exit code and signals.
    let e = Command::new(&mg).args(std::env::args_os().skip(1)).exec();
    eprintln!("mergegate: running {}: {e}", mg.display());
    ExitCode::FAILURE
}
