use super::*;

#[test]
fn ids_have_one_spelling() {
    assert_eq!(CommentId::parse("c_17"), Some(CommentId(17)));
    assert_eq!(CommentId(17).to_string(), "c_17");
    for bad in ["c_0", "c_017", "c_+1", "17", "c_", "c_x", "C_1"] {
        assert_eq!(CommentId::parse(bad), None, "{bad}");
    }
}
