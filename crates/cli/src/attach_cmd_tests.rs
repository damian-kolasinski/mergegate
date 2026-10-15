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

fn attached(name: &str) -> Attachment {
    Attachment {
        revision: 1,
        name: name.into(),
        media_type: "image/png".into(),
        video: false,
        size: 1,
        url: String::new(),
    }
}

fn rest(path: &str) -> Checked {
    Checked {
        path: path.into(),
        name: String::new(),
    }
}

#[test]
fn a_failed_upload_names_what_went_and_what_is_left() {
    let failed = || {
        CliError::new("hub_unavailable", "lost the hub")
            .hint("mg hub status")
            .exit(exit::UNAVAILABLE)
    };

    let error = unsent(
        failed(),
        "app#1",
        &[attached("a.png")],
        &[rest("b.png"), rest("c.webm")],
    );
    assert_eq!(
        error.body.message,
        "lost the hub (a.png attached before it)"
    );
    assert_eq!(
        error.body.hint.as_deref(),
        Some("mg attach app#1 b.png c.webm")
    );

    let error = unsent(failed(), "app#1", &[], &[rest("a.png")]);
    assert_eq!(error.body.message, "lost the hub");
    assert_eq!(error.body.hint.as_deref(), Some("mg attach app#1 a.png"));

    let store = CliError::new("store_failed", "disk full");
    let error = unsent(store, "app#1", &[], &[rest("a.png")]);
    assert_eq!(error.body.hint.as_deref(), Some("mg attach app#1 a.png"));
}

#[test]
fn a_refusal_that_a_retry_cannot_fix_keeps_the_hubs_hint() {
    let missing = CliError::new("item_not_found", "no item app#9").hint("mg inbox");
    let error = unsent(missing, "app#9", &[], &[rest("a.png")]);
    assert_eq!(error.body.hint.as_deref(), Some("mg inbox"));

    let closed = || CliError::new("item_closed", "app#1 is abandoned");
    let error = unsent(closed(), "app#1", &[], &[rest("a.png")]);
    assert_eq!(error.body.hint, None);

    let error = unsent(closed(), "app#1", &[attached("a.png")], &[rest("b.png")]);
    assert_eq!(
        error.body.message,
        "app#1 is abandoned (a.png attached before it; b.png not sent)"
    );
    assert_eq!(error.body.hint, None);

    let too_large = CliError::new("attachment_too_large", "b.webm is over the limit");
    let error = unsent(
        too_large,
        "app#1",
        &[attached("a.png")],
        &[rest("b.webm"), rest("c.png")],
    );
    assert_eq!(
        error.body.message,
        "b.webm is over the limit (a.png attached before it; b.webm, c.png not sent)"
    );
    assert_eq!(error.body.hint, None);
}

#[test]
fn a_local_file_that_went_missing_is_not_retried() {
    // The hub never answers an upload with attachment_unreadable; only this
    // mg does, when a checked file can't be read any more.
    let vanished = CliError::new("attachment_unreadable", "reading b.png failed");
    let error = unsent(
        vanished,
        "app#1",
        &[attached("a.png")],
        &[rest("b.png"), rest("c.png")],
    );
    assert_eq!(error.body.hint, None);
    assert_eq!(
        error.body.message,
        "reading b.png failed (a.png attached before it; b.png, c.png not sent)"
    );
}
