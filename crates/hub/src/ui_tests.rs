use super::*;

#[test]
fn only_paths_under_the_directory_are_plain() {
    assert!(is_plain("index.html"));
    assert!(is_plain("assets/main.js"));
    assert!(!is_plain("../hub.json"));
    assert!(!is_plain("assets/../../hub.json"));
    assert!(!is_plain("/etc/passwd"));
}

#[test]
fn an_etag_in_if_none_match_is_fresh() {
    let etag = "\"0123456789abcdef\"";
    let request = |value: &'static str| {
        let mut headers = HeaderMap::new();
        headers.insert(IF_NONE_MATCH, HeaderValue::from_static(value));
        headers
    };
    assert!(is_fresh(&request("\"0123456789abcdef\""), etag));
    assert!(is_fresh(&request("\"other\", \"0123456789abcdef\""), etag));
    assert!(is_fresh(&request("*"), etag));
    assert!(!is_fresh(&request("\"other\""), etag));
    assert!(!is_fresh(&HeaderMap::new(), etag));
}

#[test]
fn types_follow_the_extension() {
    assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
    assert_eq!(
        content_type("assets/main.js"),
        "text/javascript; charset=utf-8"
    );
    assert_eq!(content_type("assets/dm-sans.woff2"), "font/woff2");
    assert_eq!(content_type("assets/README"), "application/octet-stream");
}

#[test]
fn every_asset_has_a_type_and_the_index_exists() {
    assert!(ASSETS.iter().any(|a| a.path == "index.html"));
    for asset in ASSETS {
        assert_ne!(
            content_type(asset.path),
            "application/octet-stream",
            "{}",
            asset.path
        );
    }
}
