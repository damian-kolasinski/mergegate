use super::*;

#[test]
fn finds_the_next_page_in_a_link_header() {
    let link = r#"<https://api.github.com/x?page=2>; rel="next", <https://api.github.com/x?page=5>; rel="last""#;
    assert_eq!(
        next_page(link).as_deref(),
        Some("https://api.github.com/x?page=2")
    );
    let last = r#"<https://api.github.com/x?page=1>; rel="prev", <https://api.github.com/x?page=1>; rel="first""#;
    assert_eq!(next_page(last), None);
}

#[test]
fn encodes_what_a_path_cant_hold() {
    assert_eq!(encode("acme:feat/a b"), "acme%3Afeat/a%20b");
}
