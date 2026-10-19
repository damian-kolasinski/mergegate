use super::*;

#[test]
fn only_a_later_release_takes_over() {
    assert!(newer("0.4.0", "0.3.0"));
    assert!(newer("0.4.1", "0.4.0"));
    assert!(newer("1.0.0", "0.10.0"));
    assert!(newer("0.10.0", "0.9.0"));
    assert!(!newer("0.4.0", "0.4.0"));
    assert!(!newer("0.3.0", "0.4.0"));
}

#[test]
fn a_hub_between_releases_is_as_old_as_the_release_it_builds_on() {
    assert!(newer("0.4.0", "0.3.0+0123456789ab"));
    assert!(!newer("0.4.0", "0.4.0+0123456789ab"));
}

#[test]
fn a_build_between_releases_never_takes_over() {
    assert!(!newer("0.4.0+0123456789ab", "0.3.0"));
    assert!(!newer("0.5.0-rc.1", "0.4.0"));
    assert!(!newer("0.4", "0.3.0"));
    assert!(!newer("0.4.0", "unknown"));
}
