use super::*;

#[test]
fn logins_match_without_case_or_the_bot_suffix() {
    assert!(same_login(
        "chatgpt-codex-connector[bot]",
        "ChatGPT-Codex-Connector"
    ));
    assert!(same_login("codex", "codex[bot]"));
    assert!(same_login("codex[bot]", "codex[bot]"));
    assert!(!same_login("codex", "codex-review"));
    assert!(!same_login("codex[bot]x", "codex"));
}
