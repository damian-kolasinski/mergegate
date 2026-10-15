use super::*;

fn paths(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("src/f{i}.rs")).collect()
}

#[test]
fn lists_a_few_conflicting_paths_in_full() {
    assert_eq!(listed(&paths(1)), "src/f1.rs");
    assert_eq!(listed(&paths(3)), "src/f1.rs, src/f2.rs, src/f3.rs");
}

#[test]
fn counts_the_paths_past_the_first_few() {
    assert_eq!(
        listed(&paths(43)),
        "src/f1.rs, src/f2.rs, src/f3.rs and 40 more"
    );
}
