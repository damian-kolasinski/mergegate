//! Running the `tailscale` CLI.

use std::io::ErrorKind;
use std::process::{Command, Output, Stdio};

use mergegate_core::device::TailnetError;

/// Where the Mac app keeps its CLI when the operator never put it on `PATH`.
const MAC_APP: &str = "/Applications/Tailscale.app/Contents/MacOS/Tailscale";

/// Its stdout, or why it failed in the CLI's own words.
pub(crate) fn tailscale(args: &[&str]) -> Result<Vec<u8>, TailnetError> {
    let output = match spawn("tailscale", args) {
        Err(e) if e.kind() == ErrorKind::NotFound => spawn(MAC_APP, args),
        output => output,
    }
    .map_err(|e| {
        let why = match e.kind() {
            ErrorKind::NotFound => "Tailscale isn't installed: no tailscale on PATH".to_owned(),
            _ => format!("can't run tailscale: {e}"),
        };
        TailnetError::Unavailable(why)
    })?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let why = match stderr.trim() {
        "" => output.status.to_string(),
        stderr => stderr.to_owned(),
    };
    Err(TailnetError::Unavailable(format!(
        "tailscale {} failed: {why}",
        args[0]
    )))
}

fn spawn(program: &str, args: &[&str]) -> std::io::Result<Output> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
}
