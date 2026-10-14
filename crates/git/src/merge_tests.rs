use super::*;

#[test]
fn parses_an_identity_as_git_var_prints_it() {
    let ident = Ident::parse("Ada Lovelace <ada@example.com> 1700000000 +0200").unwrap();
    assert_eq!(ident.name, "Ada Lovelace");
    assert_eq!(ident.email, "ada@example.com");
    assert_eq!(ident.date, "1700000000 +0200");
}

#[test]
fn an_identity_without_an_email_is_none() {
    assert!(Ident::parse("Ada 1700000000 +0200").is_none());
}
