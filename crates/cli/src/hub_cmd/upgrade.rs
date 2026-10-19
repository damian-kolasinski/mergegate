//! A hub service left on an older `mg` by an upgrade, restarted onto this
//! one by the first command that goes through it. Until then the hub lacks
//! whatever the upgrade changed, or refuses every command when its API did,
//! and its service starts the old `mg` again at login.

use std::fs::File;
use std::time::Duration;

use hub::Home;

use super::probe::{Running, settle};
use super::{io_error, service, start, stop};
use crate::output::CliError;

/// How long to wait for a hub another `mg` just restarted, as `mg hub start`
/// waits for one.
const SETTLE: Duration = Duration::from_secs(10);

/// The hub to work through: `running`, or the one this restarted it as. None
/// once no hub runs.
pub(crate) fn onto_this_mg(home: &Home, running: Running) -> Result<Option<Running>, CliError> {
    if !due(&running) {
        return Ok(Some(running));
    }
    // Commands started together restart it once: the rest wait here, then
    // find the new hub.
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(home.root().join("upgrade.lock"))
        .and_then(|file| file.lock().map(|()| file))
        .map_err(|e| io_error("locking the hub's upgrade", e))?;
    let Some(running) = settle(home, SETTLE)? else {
        return Ok(None);
    };
    if !due(&running) {
        return Ok(Some(running));
    }
    let theirs = running.health.version.clone();
    stop::run(home)?;
    start::run(home.clone(), false, running.port())?;
    drop(lock);
    let restarted = settle(home, SETTLE)?;
    eprintln!(
        "restarted the hub, which ran mg {theirs}, on this mg {}",
        env!("MG_VERSION")
    );
    Ok(restarted)
}

/// Only a service, which an upgrade leaves pointing at the old `mg`: a hub
/// started without one, as in tests and development, runs whichever `mg`
/// started it.
fn due(running: &Running) -> bool {
    service::applies()
        && service::installed().is_some()
        && newer(env!("MG_VERSION"), &running.health.version)
}

/// `ours` is a release later than `theirs`. A build between releases, as in
/// `0.4.0+0123456789ab`, never takes over a hub, while a hub running one is
/// as old as the release it builds on.
fn newer(ours: &str, theirs: &str) -> bool {
    let release = |version: &str| {
        let mut parts = version.split('.').map(|p| p.parse::<u64>().ok());
        let parsed = (parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(parsed)
    };
    let theirs = theirs
        .split_once('+')
        .map_or(theirs, |(release, _)| release);
    matches!((release(ours), release(theirs)), (Some(ours), Some(theirs)) if ours > theirs)
}

#[cfg(test)]
#[path = "upgrade_tests.rs"]
mod tests;
