use super::*;
use crate::fake::rule;

fn of(rules: &[&Rule]) -> Rules {
    Rules::new(rules.iter().map(|&r| r.clone()).collect()).unwrap()
}

#[test]
fn reviewing_more_doesnt_loosen() {
    let (r1, r2) = (
        rule(1, Tier::Review, "src/**"),
        rule(2, Tier::Trusted, "**"),
    );
    let r3 = rule(3, Tier::Review, "docs/**");
    let applied = of(&[&r1, &r2]);
    for next in [
        of(&[&r1, &r2]),
        of(&[&r3, &r1, &r2]),
        of(&[&r1, &r2, &r3]),
        of(&[&r1]),
        of(&[&r3, &r1]),
    ] {
        assert!(!loosens(&applied, &next), "{next:?}");
    }
}

#[test]
fn anything_else_loosens() {
    let (r1, r2) = (
        rule(1, Tier::Review, "src/**"),
        rule(2, Tier::Trusted, "*.md"),
    );
    let r3 = rule(3, Tier::Trusted, "**");
    let wider = Rule {
        path: Some("**".into()),
        ..r2.clone()
    };
    let applied = of(&[&r1, &r2]);
    for next in [
        of(&[&r1, &r2, &r3]),
        of(&[&r3, &r1, &r2]),
        of(&[&r2]),
        of(&[&r2, &r1]),
        of(&[&r1, &wider]),
    ] {
        assert!(loosens(&applied, &next), "{next:?}");
    }
}
