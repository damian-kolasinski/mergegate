//! Diffs that look like they hold a secret, which never leave the machine
//! for Jev's hosted API. A name is enough: a value can't be told from any
//! other string, and a file that only mentions a password going unjudged
//! costs far less than a key that was sent.

/// Words that are a secret's name, alone or ending one written as a single
/// word, as `PGPASSWORD`, `GHTOKEN` or `PRIVATEKEY` are. `pwd` isn't one: it
/// is the shell's working directory far more often than a password.
const ENDINGS: &[&str] = &[
    "password",
    "passwd",
    "passphrase",
    "secret",
    "credential",
    "token",
    "bearer",
    "dsn",
    "apikey",
    "privatekey",
    "accesskey",
];

/// Words that name a secret when `key` or `keys` follows them, as in
/// `API_KEY`, `apiKey` or a PEM's `PRIVATE KEY`.
const KEY_OF: &[&str] = &["api", "private", "access"];

pub(crate) fn looks_secret(diff: &str) -> bool {
    let words = words(diff);
    words.iter().enumerate().any(|(i, word)| {
        names_secret(word)
            || KEY_OF.contains(&word.as_str())
                && words
                    .get(i + 1)
                    .is_some_and(|next| next == "key" || next == "keys")
    })
}

fn names_secret(word: &str) -> bool {
    let singular = word.strip_suffix('s').unwrap_or(word);
    ENDINGS.iter().any(|ending| singular.ends_with(ending))
}

/// Lowercased words, split at anything but a letter or digit and inside
/// camel case, so `DB_PASSWORD`, `db-password` and `dbPassword` all hold
/// `password`. A word boundary in a regex would see `_` as part of a word.
fn words(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut words = Vec::new();
    let mut word = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        let prev = i.checked_sub(1).map(|j| chars[j]);
        let next = chars.get(i + 1);
        // `dbPassword` and `oauth2Secret` split before the capital, `APIKey`
        // before the K.
        let starts = c.is_uppercase()
            && prev.is_some_and(|p| {
                p.is_lowercase()
                    || p.is_numeric()
                    || p.is_uppercase() && next.is_some_and(|n| n.is_lowercase())
            });
        if starts && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        word.extend(c.to_lowercase());
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

#[cfg(test)]
#[path = "secrets_tests.rs"]
mod tests;
