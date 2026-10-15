use super::*;

fn app1() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

#[test]
fn names_the_item_then_gives_the_guide() {
    assert_eq!(
        description(&app1(), None),
        "Reviewed in mergegate as app#1."
    );
    assert_eq!(
        description(&app1(), Some(" \n")),
        "Reviewed in mergegate as app#1."
    );
    assert_eq!(
        description(&app1(), Some("\nSigns users in.\n")),
        "Reviewed in mergegate as app#1.\n\nSigns users in."
    );
}

#[test]
fn a_guide_too_long_for_github_gives_only_its_intent() {
    let files = "- `src/a.rs` — why it matters\n".repeat(3_000);
    let guide = format!("Signs users in.\n\n## Files\n{files}");
    assert!(guide.len() > MAX_BODY);
    assert_eq!(
        description(&app1(), Some(&guide)),
        "Reviewed in mergegate as app#1.\n\nSigns users in.\n\n\
         _The guide is too long for GitHub; read it in mergegate._"
    );
}

#[test]
fn an_intent_too_long_for_github_is_cut_short() {
    for guide in ["é".repeat(MAX_BODY), "## Files\n".repeat(MAX_BODY)] {
        let body = description(&app1(), Some(&guide));
        assert!(body.len() <= MAX_BODY, "{}", body.len());
        assert!(
            body.ends_with("\n\n_The guide is too long for GitHub; read it in mergegate._"),
            "{}",
            &body[body.len() - 80..]
        );
    }
}
