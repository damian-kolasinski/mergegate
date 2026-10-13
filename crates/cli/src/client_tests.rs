use super::segment;

#[test]
fn segments_escape_everything_but_unreserved_characters() {
    assert_eq!(segment("app-2"), "app-2");
    assert_eq!(segment("../x y/ż"), "%2E%2E%2Fx%20y%2F%C5%BC");
}
