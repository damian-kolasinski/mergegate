use super::*;

#[test]
fn parses_the_modes_git_writes() {
    for mode in FileMode::ALL {
        assert_eq!(FileMode::parse(mode.as_str()), Some(mode));
    }
    for other in ["000000", "040000", "100664", "755", ""] {
        assert_eq!(FileMode::parse(other), None, "{other:?}");
    }
}
