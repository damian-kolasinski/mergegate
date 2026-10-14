//! What can be attached: images and videos a browser shows inline, told
//! apart by their bytes. Anything else is refused, SVG and HTML above all:
//! the hub serves attachments from its own origin, where a script in one
//! would run as the UI.

/// Per file. A demo is a few screenshots or a short recording.
pub const MAX_SIZE: u64 = 100 * 1024 * 1024;

/// How many leading bytes [`Media::sniff`] reads.
pub const SNIFF_LEN: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Media {
    Png,
    Jpeg,
    Gif,
    Webp,
    /// Any ISO base media file, QuickTime's `.mov` too: Chrome plays the
    /// H.264 inside either.
    Mp4,
    Webm,
}

/// ISO-BMFF brands of still images (AVIF, HEIC): they share MP4's `ftyp`
/// box, but a video element can't show them.
const HEIF_BRANDS: [&[u8; 4]; 8] = [
    b"avif", b"avis", b"heic", b"heix", b"heim", b"heis", b"mif1", b"msf1",
];

impl Media {
    pub const ALL: [Self; 6] = [
        Self::Png,
        Self::Jpeg,
        Self::Gif,
        Self::Webp,
        Self::Mp4,
        Self::Webm,
    ];

    /// The type of the file starting with `head`, if it is one of these.
    pub fn sniff(head: &[u8]) -> Option<Self> {
        let at =
            |offset: usize, magic: &[u8]| head.get(offset..offset + magic.len()) == Some(magic);
        if at(0, b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if at(0, b"\xff\xd8\xff") {
            Some(Self::Jpeg)
        } else if at(0, b"GIF87a") || at(0, b"GIF89a") {
            Some(Self::Gif)
        } else if at(0, b"RIFF") && at(8, b"WEBP") {
            Some(Self::Webp)
        } else if at(4, b"ftyp") && !HEIF_BRANDS.iter().any(|brand| at(8, *brand)) {
            Some(Self::Mp4)
        } else if at(0, b"\x1a\x45\xdf\xa3") {
            // EBML, which Matroska shares; Chrome plays both.
            Some(Self::Webm)
        } else {
            None
        }
    }

    /// The MIME type it is served with, which the store keeps too.
    pub fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
            Self::Mp4 => "video/mp4",
            Self::Webm => "video/webm",
        }
    }

    pub fn from_mime(mime: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.mime() == mime)
    }

    pub fn is_video(self) -> bool {
        matches!(self, Self::Mp4 | Self::Webm)
    }
}

/// A file name as `mg` sends it: one path component, short, printable.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name != "."
        && name != ".."
        && !name
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_control())
}

#[cfg(test)]
#[path = "media_tests.rs"]
mod tests;
