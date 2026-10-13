use super::*;

#[test]
fn parses_the_names_it_prints() {
    for backend in Backend::ALL {
        assert_eq!(Backend::parse(backend.as_str()), Some(backend));
    }
    assert_eq!(Backend::parse("Local"), None);
}
