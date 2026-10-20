use super::{Hub, incompatible, segment};

#[test]
fn segments_escape_everything_but_unreserved_characters() {
    assert_eq!(segment("app-2"), "app-2");
    assert_eq!(segment("../x y/ż"), "%2E%2E%2Fx%20y%2F%C5%BC");
}

fn hub(version: &str, connected: bool) -> Hub {
    Hub {
        url: "http://mac-mini:7800".into(),
        health: api::hub::Health {
            version: version.into(),
            api_version: 0xabcd,
            schema_version: 1,
            pid: 1,
            home: String::new(),
        },
        connected,
    }
}

#[test]
fn an_incompatible_hub_names_both_mgs() {
    let (message, _) = incompatible(&hub("0.6.2", true), "0.7.0");
    assert!(
        message.starts_with("the hub runs mg 0.6.2, which speaks API 0000abcd; this is mg 0.7.0"),
        "{message}"
    );
}

#[test]
fn the_older_side_is_the_one_to_upgrade() {
    let (_, hint) = incompatible(&hub("0.6.2", true), "0.7.0");
    assert_eq!(
        hint,
        "upgrade mg on the hub's machine to 0.7.0, then restart the hub there: mg hub stop && mg hub start"
    );
    let (_, hint) = incompatible(&hub("0.7.0", true), "0.6.2");
    assert_eq!(hint, "install mg 0.7.0 here, the hub's mg");
}

#[test]
fn a_hub_on_this_machine_restarts_on_this_mg() {
    let (_, hint) = incompatible(&hub("0.6.2", false), "0.7.0");
    assert_eq!(hint, "mg hub stop && mg hub start");
}
