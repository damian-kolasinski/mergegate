use super::*;

#[test]
fn reads_a_host_an_address_or_a_url() {
    let cases = [
        ("mac-mini", "http://mac-mini:7800"),
        ("mac-mini:7801", "http://mac-mini:7801"),
        ("mac-mini.tail0.ts.net", "http://mac-mini.tail0.ts.net:7800"),
        ("100.64.0.1", "http://100.64.0.1:7800"),
        ("100.64.0.1:9000", "http://100.64.0.1:9000"),
        ("http://mac-mini:7800/", "http://mac-mini:7800"),
        ("http://mac-mini", "http://mac-mini:7800"),
        ("fd7a::1", "http://[fd7a::1]:7800"),
        ("[fd7a::1]", "http://[fd7a::1]:7800"),
        ("[fd7a::1]:7801", "http://[fd7a::1]:7801"),
    ];
    for (hub, expected) in cases {
        assert_eq!(url(hub).unwrap(), expected, "{hub}");
    }
}

#[test]
fn refuses_anything_else() {
    for hub in [
        "",
        "https://mac-mini",
        "mac-mini:port",
        "mac-mini:99999",
        "http://mac-mini/api",
        "user@mac-mini",
        "not a url",
        "mac\tmini",
        ":7800",
        "http://:7800",
        "[]",
        "[fd7a::1",
        "mac-mini::7800",
        "a:b:c",
        "[mac-mini]",
        "[mac-mini]:7801",
        "[]:7801",
    ] {
        let err = url(hub).unwrap_err();
        assert_eq!(err.body.code, "invalid_hub", "{hub}");
    }
}
