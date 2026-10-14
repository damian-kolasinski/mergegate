use super::*;

#[test]
fn sniffs_media_by_its_bytes() {
    let cases: &[(&[u8], Option<Media>)] = &[
        (b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR", Some(Media::Png)),
        (b"\xff\xd8\xff\xe0\0\x10JFIF", Some(Media::Jpeg)),
        (b"GIF89a\x01\0\x01\0", Some(Media::Gif)),
        (b"GIF87a\x01\0\x01\0", Some(Media::Gif)),
        (b"RIFF\x24\0\0\0WEBPVP8 ", Some(Media::Webp)),
        (b"\0\0\0\x20ftypisom\0\0", Some(Media::Mp4)),
        (b"\0\0\0\x14ftypqt  \0\0", Some(Media::Mp4)),
        (b"\0\0\0\x1cftypavif\0\0", None),
        (b"\0\0\0\x18ftypheic\0\0", None),
        (b"\x1a\x45\xdf\xa3\x9f\x42\x86\x81", Some(Media::Webm)),
        (b"<svg xmlns=\"http://www.w3.org/2000/svg\">", None),
        (b"<!doctype html><script>", None),
        (b"RIFF\x24\0\0\0WAVEfmt ", None),
        (b"%PDF-1.7", None),
        (b"\x89PN", None),
        (b"", None),
    ];
    for (head, media) in cases {
        assert_eq!(Media::sniff(head), *media, "{head:?}");
    }
}

#[test]
fn mime_types_round_trip() {
    for media in Media::ALL {
        assert_eq!(Media::from_mime(media.mime()), Some(media));
    }
    assert_eq!(Media::from_mime("image/svg+xml"), None);
}

#[test]
fn names_are_one_printable_component() {
    for name in [
        "demo.mp4",
        "Screen Shot 2026-10-14 at 10.00.png",
        "ünïcode.gif",
    ] {
        assert!(valid_name(name), "{name}");
    }
    let long = "a".repeat(256);
    for name in [
        "",
        ".",
        "..",
        "a/b.png",
        "a\\b.png",
        "a\nb.png",
        long.as_str(),
    ] {
        assert!(!valid_name(name), "{name:?}");
    }
}
