use super::*;

#[test]
fn parses_what_it_displays() {
    let id = ItemId::parse("my-app#42").unwrap();
    assert_eq!(id.project().as_str(), "my-app");
    assert_eq!(id.number(), 42);
    assert_eq!(id.to_string(), "my-app#42");
}

#[test]
fn refuses_malformed_ids() {
    for id in [
        "app", "app#", "#1", "app#0", "app#01", "app#+1", "app#x", "app#1#2",
    ] {
        assert!(ItemId::parse(id).is_err(), "{id:?}");
    }
    assert!(matches!(
        ItemId::parse("App#1"),
        Err(IdError::InvalidProject(_))
    ));
    assert_eq!(
        ItemId::parse("app").unwrap_err().to_string(),
        "\"app\" is not an item ID; IDs look like app#42"
    );
}
