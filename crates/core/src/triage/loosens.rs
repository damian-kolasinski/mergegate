//! Whether a change to the rules needs the operator's approval.

use super::{Rule, Rules, Tier};

/// Whether `next` could put in `trusted` a file `applied` reviews. Adding a
/// `review` rule anywhere only moves files into `review`, and removing a
/// `trusted` one leaves its files to the rules after it, which hid nothing
/// new; anything else (a new or changed `trusted` rule, a removed or changed
/// `review` one, a reorder) might.
pub fn loosens(applied: &Rules, next: &Rules) -> bool {
    let has = |rules: &Rules, rule: &Rule| rules.rules().any(|r| r.id == rule.id);
    let kept_next = next
        .rules()
        .filter(|r| r.tier == Tier::Trusted || has(applied, r));
    let kept_applied = applied
        .rules()
        .filter(|r| r.tier == Tier::Review || has(next, r));
    !kept_next.eq(kept_applied)
}

#[cfg(test)]
#[path = "loosens_tests.rs"]
mod tests;
