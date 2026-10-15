use super::*;

#[test]
fn a_secrets_name_in_any_case_is_enough() {
    for diff in [
        "+let password = form.value;",
        "+DATABASE_PASSWORD=hunter2",
        "+let dbPassword = env(x);",
        "+  client_secret: x",
        "+Secret: x",
        "+credentials = load()",
        "+API_KEY=sk-1",
        "+apiKey: k",
        "+let APIKey = k",
        "+headers['x-api-key'] = k",
        "+AWS_ACCESS_KEY_ID=AKIA",
        "+-----BEGIN RSA PRIVATE KEY-----",
        "+SENTRY_DSN=https://x@y/1",
        "+GITHUB_TOKEN=ghp_1",
        "+refreshTokens: []",
        "+Authorization: Bearer abc",
        "+ssh_passphrase = x",
        "+export PGPASSWORD=x",
        "+GHTOKEN=x",
        "+PRIVATEKEY=x",
        "+let secrets = vault();",
    ] {
        assert!(looks_secret(diff), "{diff}");
    }
}

#[test]
fn a_word_that_only_contains_one_is_not() {
    for diff in [
        "+fn secretary() {}",
        "+let apiKeyboard = 3;",
        "+let keyApi = 3;",
        "+dsname",
        "+fn login(user: &str) {}",
        "+private fn key() {}",
        "+cd $(pwd)",
        "+let tokenizer = Lexer::new();",
        "+passwordless_login()",
    ] {
        assert!(!looks_secret(diff), "{diff}");
    }
}

#[test]
fn splits_snake_kebab_and_camel_case() {
    assert_eq!(
        words("DB_PASSWORD x-api-key dbPassword APIKey v2Name"),
        [
            "db", "password", "x", "api", "key", "db", "password", "api", "key", "v2", "name"
        ]
    );
}
