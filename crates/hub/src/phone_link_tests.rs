use super::link;

#[test]
fn links_the_page_on_the_tailnet() {
    let link = link("http://my-mac:7800", "#/items/app%233").ok();
    assert_eq!(link.as_deref(), Some("http://my-mac:7800/#/items/app%233"));
}

#[test]
fn refuses_anything_but_a_ui_route() {
    for route in [
        "",
        "/api/health",
        "#/a b",
        "#/\n",
        &format!("#/{}", "x".repeat(511)),
    ] {
        assert!(link("http://my-mac:7800", route).is_err(), "{route:?}");
    }
}
