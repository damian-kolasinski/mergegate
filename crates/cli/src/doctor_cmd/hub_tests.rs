use api::doctor::CheckStatus;

use super::versioned;

#[test]
fn a_hub_of_this_version_is_ok() {
    let check = versioned("http://127.0.0.1:7777", "1.2.0", "1.2.0");
    assert_eq!(check.status, CheckStatus::Ok);
    assert_eq!(check.message, "running at http://127.0.0.1:7777");
    assert_eq!(check.fix, None);
}

#[test]
fn a_hub_of_another_version_warns_and_says_how_to_restart_it() {
    let check = versioned("http://127.0.0.1:7777", "1.1.0", "1.2.0");
    assert_eq!(check.status, CheckStatus::Warn);
    assert_eq!(
        check.message,
        "running at http://127.0.0.1:7777, but as mg 1.1.0; this is mg 1.2.0"
    );
    assert_eq!(check.fix.as_deref(), Some("mg hub stop && mg hub start"));
}
