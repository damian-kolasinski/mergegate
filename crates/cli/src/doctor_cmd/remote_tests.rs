use api::doctor::CheckStatus;

use super::judge;

const HUB: &str = "http://127.0.0.1:7777/git/";
const APP: (&str, &str) = ("app", "http://127.0.0.1:7777/git/app");

fn status_and_fix(
    project: Option<(&str, &str)>,
    remote: Option<&str>,
) -> Option<(CheckStatus, Option<String>)> {
    judge(project, remote, HUB).map(|c| (c.status, c.fix))
}

#[test]
fn a_project_clone_pushing_to_its_project_is_ok() {
    let check = judge(Some(APP), Some(APP.1), HUB).unwrap();
    assert_eq!(check.status, CheckStatus::Ok);
    assert_eq!(check.message, "mergegate pushes to project app");
}

#[test]
fn a_project_clone_without_the_right_remote_needs_init() {
    let fix = Some((CheckStatus::Warn, Some("mg init".to_owned())));
    assert_eq!(status_and_fix(Some(APP), None), fix);
    assert_eq!(
        status_and_fix(Some(APP), Some("http://127.0.0.1:9999/git/app")),
        fix
    );
}

#[test]
fn a_clone_the_hub_cant_place_is_judged_by_its_remote() {
    assert_eq!(
        status_and_fix(None, Some("http://127.0.0.1:7777/git/other")),
        Some((CheckStatus::Ok, None))
    );
    assert_eq!(
        status_and_fix(None, Some("http://127.0.0.1:9999/git/app")),
        Some((CheckStatus::Warn, Some("mg init --project NAME".to_owned())))
    );
}

#[test]
fn a_repo_with_neither_a_project_nor_a_remote_is_skipped() {
    assert_eq!(status_and_fix(None, None), None);
}
