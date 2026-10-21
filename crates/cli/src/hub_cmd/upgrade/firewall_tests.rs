use api::doctor::Check;

use super::*;

fn check(name: &str, status: CheckStatus, fix: Option<&str>) -> Check {
    Check {
        name: name.into(),
        status,
        message: format!("{name} says so"),
        fix: fix.map(str::to_owned),
    }
}

#[test]
fn names_the_firewall_and_its_fix_and_nothing_else() {
    let report = Report {
        checks: vec![
            check("tailnet", CheckStatus::Warn, Some("restart the hub")),
            check(
                "firewall",
                CheckStatus::Warn,
                Some("sudo socketfilterfw --add mg"),
            ),
        ],
    };
    assert_eq!(
        warnings(report),
        ["warning: firewall says so; on this machine, run: sudo socketfilterfw --add mg"]
    );
}

#[test]
fn a_firewall_that_lets_the_tailnet_in_says_nothing() {
    let report = Report {
        checks: vec![check("firewall", CheckStatus::Ok, None)],
    };
    assert!(warnings(report).is_empty());
}
