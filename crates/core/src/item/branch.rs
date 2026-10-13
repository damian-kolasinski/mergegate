/// Whether `name` is a branch name git accepts (`git check-ref-format
/// --branch`), so it can be put in a ref and on a command line as is.
pub fn valid_branch(name: &str) -> bool {
    let forbidden = |c: char| c.is_ascii_control() || " ~^:?*[\\".contains(c);
    !name.is_empty()
        && name != "HEAD"
        && name != "@"
        && !name.starts_with('-')
        && !name.ends_with('.')
        && !name.contains("..")
        && !name.contains("@{")
        && !name.chars().any(forbidden)
        && name
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock"))
}

#[cfg(test)]
#[path = "branch_tests.rs"]
mod tests;
