use std::path::Path;

use api::doctor::CheckStatus;

use super::judge;
use crate::hub_cmd::service::{Installed, Manager};

fn unit(exe: Option<&str>) -> Installed {
    Installed {
        manager: Manager::Launchd,
        unit: "/Users/op/Library/LaunchAgents/dev.mergegate.hub.plist".into(),
        exe: exe.map(str::to_owned),
    }
}

#[test]
fn no_unit_warns_with_the_command_that_installs_one() {
    let stopped = judge(None, None, false);
    assert_eq!(stopped.status, CheckStatus::Warn);
    assert_eq!(stopped.fix.as_deref(), Some("mg hub start"));

    let running = judge(None, None, true);
    assert_eq!(running.fix.as_deref(), Some("mg hub stop && mg hub start"));
}

#[test]
fn a_unit_that_runs_this_mg_is_ok() {
    let check = judge(
        Some(&unit(Some("/opt/mg/bin/mg"))),
        Some(Path::new("/opt/mg/bin/mg")),
        true,
    );
    assert_eq!(check.status, CheckStatus::Ok);
    assert_eq!(
        check.message,
        "launchd starts the hub at login: /Users/op/Library/LaunchAgents/dev.mergegate.hub.plist"
    );
}

#[test]
fn a_unit_that_runs_another_mg_warns() {
    let check = judge(
        Some(&unit(Some("/old/mg"))),
        Some(Path::new("/opt/mg/bin/mg")),
        true,
    );
    assert_eq!(check.status, CheckStatus::Warn);
    assert_eq!(
        check.message,
        "the hub's service runs /old/mg, not this mg (/opt/mg/bin/mg)"
    );
    assert_eq!(check.fix.as_deref(), Some("mg hub stop && mg hub start"));
}

#[test]
fn a_link_to_this_mg_is_this_mg() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("mg");
    std::fs::write(&real, "").unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let check = judge(Some(&unit(Some(link.to_str().unwrap()))), Some(&real), true);
    assert_eq!(check.status, CheckStatus::Ok);
}

#[test]
fn a_unit_mg_did_not_write_warns() {
    let check = judge(Some(&unit(None)), Some(Path::new("/opt/mg/bin/mg")), true);
    assert_eq!(check.status, CheckStatus::Warn);
    assert_eq!(
        check.message,
        "/Users/op/Library/LaunchAgents/dev.mergegate.hub.plist isn't a unit mg wrote"
    );
}
