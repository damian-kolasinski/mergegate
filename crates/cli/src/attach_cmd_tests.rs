use super::*;

#[test]
fn prints_sizes_in_the_largest_fitting_unit() {
    assert_eq!(bytes(0), "0 B");
    assert_eq!(bytes(1023), "1023 B");
    assert_eq!(bytes(1536), "1.5 KB");
    assert_eq!(bytes(MAX_SIZE), "100.0 MB");
}

#[test]
fn checks_files_before_anything_is_sent() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("shot.png");
    std::fs::write(&png, b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR").unwrap();
    let svg = dir.path().join("logo.svg");
    std::fs::write(&svg, b"<svg xmlns='http://www.w3.org/2000/svg'/>").unwrap();

    let checked = check(std::slice::from_ref(&png)).unwrap();
    assert_eq!(checked[0].name, "shot.png");
    let code = |path: &Path| check(&[path.to_owned()]).err().unwrap().body.code;
    assert_eq!(code(&svg), "unsupported_media");
    assert_eq!(
        code(&dir.path().join("missing.png")),
        "attachment_unreadable"
    );
    assert_eq!(code(Path::new("/")), "invalid_name");
}

#[test]
fn a_failed_upload_names_what_went_and_what_is_left() {
    let attached = |name: &str| Attachment {
        revision: 1,
        name: name.into(),
        media_type: "image/png".into(),
        video: false,
        size: 1,
        url: String::new(),
    };
    let rest = |path: &str| Checked {
        path: path.into(),
        name: String::new(),
    };
    let failed = || CliError::new("not_found", "app#1 is gone");

    let error = unsent(
        failed(),
        "app#1",
        &[attached("a.png")],
        &[rest("b.png"), rest("c.webm")],
    );
    assert_eq!(
        error.body.message,
        "app#1 is gone (a.png attached before it)"
    );
    assert_eq!(
        error.body.hint.as_deref(),
        Some("mg attach app#1 b.png c.webm")
    );

    let error = unsent(failed(), "app#1", &[], &[rest("a.png")]);
    assert_eq!(error.body.message, "app#1 is gone");
    assert_eq!(error.body.hint.as_deref(), Some("mg attach app#1 a.png"));
}
