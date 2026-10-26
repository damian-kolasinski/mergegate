use super::sanitize;

#[test]
fn selected_prose_loses_credentials_paths_and_contact_details() {
    let input = "failure request=42\nAuthorization: Bearer very-secret\nCookie: session=secret\n{\"api_key\": \"hunter2\"}\nGH_TOKEN=ghp_abc123\nhttps://user:pass@example.com/private?q=secret\n/Users/alice/project/file.rs\nC:\\Users\\alice\\secret.txt\nalice@example.com\ngithub_pat_12345\n-----BEGIN PRIVATE KEY-----\nsecret\n-----END PRIVATE KEY-----\n";
    let output = sanitize(input);
    assert!(output.contains("failure request=42"), "{output}");
    for secret in [
        "very-secret",
        "session=secret",
        "hunter2",
        "ghp_abc123",
        "user:pass",
        "alice",
        "github_pat_12345",
        "PRIVATE KEY",
        "secret",
    ] {
        assert!(!output.contains(secret), "{secret} remains in {output}");
    }
    assert_eq!(sanitize(&output), output);
}

#[test]
fn keeps_useful_context_and_unicode_but_removes_terminal_controls() {
    assert_eq!(
        sanitize("żółw\trequest_failed status=500\n\u{1b}[31m"),
        "żółw\trequest_failed status=500\n[31m"
    );
}
