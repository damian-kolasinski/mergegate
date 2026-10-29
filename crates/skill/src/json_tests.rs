use super::Json;

#[test]
fn keeps_the_order_of_keys() {
    let text = "{\n  \"zebra\": 1,\n  \"apple\": {\n    \"y\": [true, null, \"s\"],\n    \"b\": 2.5\n  }\n}\n";
    let json = Json::parse(text).unwrap();
    assert_eq!(
        json.print(),
        text.replace(
            "[true, null, \"s\"]",
            "[\n      true,\n      null,\n      \"s\"\n    ]"
        )
    );
}

#[test]
fn adds_a_missing_entry_last() {
    let mut json = Json::parse(r#"{"b": 1, "a": 2}"#).unwrap();
    json.entry("hooks").unwrap();
    json.remove("b");
    assert_eq!(json.print(), "{\n  \"a\": 2,\n  \"hooks\": {}\n}\n");
}

#[test]
fn an_entry_needs_an_object() {
    let mut json = Json::parse("[1]").unwrap();
    assert!(json.entry("hooks").is_none());
}
