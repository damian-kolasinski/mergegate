use super::UiLink;
use crate::hub_cmd::qr;

#[test]
fn opens_the_inbox_by_the_tailnet_name() {
    let link = UiLink::tailnet("http://mac-mini-appunite:7800");
    assert_eq!(link.url, "http://mac-mini-appunite:7800/#/");
}

#[test]
fn a_terminal_gets_a_qr_code_of_the_tailnet_link() {
    let link = UiLink::tailnet("http://mac-mini-appunite:7800");
    assert_eq!(
        link.render(true),
        format!(
            "open the review UI: http://mac-mini-appunite:7800/#/\n\
             or scan it with a phone on the tailnet:\n{}",
            qr::render("http://mac-mini-appunite:7800/#/")
        )
    );
    assert_eq!(
        link.render(false),
        "open the review UI: http://mac-mini-appunite:7800/#/"
    );
}

#[test]
fn a_local_link_has_no_qr_code_since_no_phone_reaches_it() {
    let link = UiLink::local(7800);
    assert_eq!(link.url, "http://localhost:7800/#/");
    assert_eq!(
        link.render(true),
        "open the review UI: http://localhost:7800/#/"
    );
}
