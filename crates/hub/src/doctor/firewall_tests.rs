use super::*;

const ON: &str = "Firewall is enabled. (State = 1)\n";
const OFF: &str = "Firewall is disabled. (State = 0)\n";
const OPEN: &str = "Firewall has block all state set to disabled.\n";
const CLOSED: &str = "Firewall has block all state set to enabled.\n";
const SIGNED: &str = "Automatically allow built-in signed software ENABLED.\nAutomatically allow downloaded signed software ENABLED.\n";
const UNSIGNED: &str = "Automatically allow built-in signed software ENABLED.\nAutomatically allow downloaded signed software DISABLED.\n";
const APPS: &str = "Total number of apps = 3 \n\
1 : /Users/me/.local/share/mise/installs/mg/0.3.0/mg \n             (Allow incoming connections)\n\
2 : /Applications/Foo.app/Contents/Helpers/Foo (Service).app \n             (Block incoming connections)\n\
3 : /opt/mg/bin/mg \n             (Block incoming connections)\n";

fn firewall(apps: Vec<App>) -> Firewall {
    Firewall {
        on: true,
        block_all: false,
        allow_signed: true,
        apps,
    }
}

fn app(path: &str, allowed: bool) -> App {
    App {
        path: path.into(),
        allowed,
    }
}

#[test]
fn parses_what_socketfilterfw_prints() {
    assert_eq!(
        Firewall::parse(ON, OPEN, SIGNED, APPS),
        Some(firewall(vec![
            app("/Users/me/.local/share/mise/installs/mg/0.3.0/mg", true),
            app(
                "/Applications/Foo.app/Contents/Helpers/Foo (Service).app",
                false
            ),
            app("/opt/mg/bin/mg", false),
        ]))
    );
    let off = Firewall::parse(OFF, CLOSED, UNSIGNED, "Total number of apps = 0 \n").unwrap();
    assert!(!off.on);
    assert!(off.block_all);
    assert!(!off.allow_signed);
    assert!(off.apps.is_empty());
    assert_eq!(
        Firewall::parse("Firewall is confused.\n", OPEN, SIGNED, APPS),
        None
    );
}

#[test]
fn an_off_firewall_needs_no_check() {
    let off = Firewall {
        on: false,
        ..firewall(vec![])
    };
    assert_eq!(judge(&off, Path::new("/opt/mg/bin/mg"), false), None);
}

#[test]
fn a_rule_decides_for_the_binary_it_names() {
    let rules = firewall(vec![app("/a/mg", true), app("/b/mg", false)]);
    let allowed = judge(&rules, Path::new("/a/mg"), false).unwrap();
    assert_eq!(allowed.name, "firewall");
    assert_eq!(allowed.status, CheckStatus::Ok);
    let blocked = judge(&rules, Path::new("/b/mg"), true).unwrap();
    assert_eq!(blocked.status, CheckStatus::Fail);
    assert_eq!(
        blocked.fix.as_deref(),
        Some("sudo /usr/libexec/ApplicationFirewall/socketfilterfw --unblockapp /b/mg")
    );
}

#[test]
fn an_unlisted_binary_is_allowed_only_as_signed_software() {
    let rules = firewall(vec![app("/old/mg", true)]);
    let new = Path::new("/new version/mg");
    assert_eq!(judge(&rules, new, true).unwrap().status, CheckStatus::Ok);
    let unsigned = judge(&rules, new, false).unwrap();
    assert_eq!(unsigned.status, CheckStatus::Warn);
    assert_eq!(
        unsigned.fix.as_deref(),
        Some(
            "sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add '/new version/mg' && sudo /usr/libexec/ApplicationFirewall/socketfilterfw --unblockapp '/new version/mg'"
        )
    );
    let strict = Firewall {
        allow_signed: false,
        ..firewall(vec![])
    };
    assert_eq!(judge(&strict, new, true).unwrap().status, CheckStatus::Warn);
}

#[test]
fn blocking_everything_fails_whatever_the_rules() {
    let closed = Firewall {
        block_all: true,
        ..firewall(vec![app("/a/mg", true)])
    };
    assert_eq!(
        judge(&closed, Path::new("/a/mg"), true).unwrap().status,
        CheckStatus::Fail
    );
}
