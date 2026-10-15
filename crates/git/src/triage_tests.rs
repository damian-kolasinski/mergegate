use super::*;

#[test]
fn parses_check_attr_output() {
    let output = "a.ts\0linguist-generated\0set\0\
                  b.ts\0linguist-generated\0true\0\
                  c.ts\0linguist-generated\0unspecified\0\
                  d.ts\0linguist-generated\0false\0\
                  e f.ts\0linguist-generated\0unset\0";
    assert_eq!(
        parse(output, 5),
        Some(vec![true, true, false, false, false])
    );
    assert_eq!(parse("a\0linguist-generated\0set\0", 2), None);
    assert_eq!(parse("a\0linguist-generated\0set", 1), None);
}
