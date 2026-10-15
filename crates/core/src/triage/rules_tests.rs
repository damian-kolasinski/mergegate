use super::*;

fn rule(id: u32) -> Rule {
    Rule {
        id: RuleId::new(id),
        tier: Tier::Trusted,
        path: Some("**/*.lock".into()),
        kind: None,
        max_lines: None,
        diff_matches: None,
        reason: "lockfiles".into(),
    }
}

#[test]
fn rule_ids_have_one_spelling() {
    for (id, parsed) in [
        ("R1", Some(1)),
        ("R42", Some(42)),
        ("R0", None),
        ("R01", None),
        ("r1", None),
        ("R", None),
        ("R1a", None),
        ("R+1", None),
        ("R4294967295", Some(u32::MAX)),
        ("R99999999999", None),
    ] {
        assert_eq!(RuleId::parse(id).map(RuleId::number), parsed, "{id}");
    }
    assert_eq!(RuleId::new(7).to_string(), "R7");
}

#[test]
fn checks_each_rule() {
    let no_matcher = Rule {
        path: None,
        ..rule(2)
    };
    let no_reason = Rule {
        reason: "  ".into(),
        ..rule(2)
    };
    let bad_glob = Rule {
        path: Some("src/[".into()),
        ..rule(2)
    };
    let bad_regex = Rule {
        diff_matches: Some("(".into()),
        ..rule(2)
    };
    let cases = [
        (vec![rule(1), rule(1)], "R1 appears twice"),
        (vec![rule(1), no_matcher], "R2 has no matcher"),
        (vec![no_reason], "R2 has no reason"),
        (vec![bad_glob], "R2's path \"src/[\" isn't a valid glob"),
        (
            vec![bad_regex],
            "R2's diff_matches \"(\" isn't a valid regex",
        ),
    ];
    for (rules, error) in cases {
        let e = Rules::new(rules).unwrap_err().to_string();
        assert!(e.starts_with(error), "{e:?} doesn't start with {error:?}");
    }
}

#[test]
fn equal_rules_compare_equal_whatever_was_compiled() {
    let a = Rules::new(vec![rule(1), rule(2)]).unwrap();
    assert_eq!(a, Rules::new(vec![rule(1), rule(2)]).unwrap());
    assert_ne!(a, Rules::new(vec![rule(2), rule(1)]).unwrap());
    assert_ne!(a, Rules::default());
}

#[test]
fn summarizes_every_matcher() {
    let rule = Rule {
        kind: Some(Kind::Generated),
        max_lines: Some(20),
        diff_matches: Some("^version".into()),
        ..rule(1)
    };
    assert_eq!(
        rule.summary(),
        "`**/*.lock`, generated, ≤ 20 lines, diff matches `^version`"
    );
}
