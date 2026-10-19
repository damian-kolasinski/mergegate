use std::io::{Cursor, Read};

use super::*;

/// The archive with each file's bytes in place of its part.
fn build(files: &[(&str, &[u8])], modified: Modified) -> Vec<u8> {
    let entries: Vec<Entry> = files
        .iter()
        .map(|(name, bytes)| Entry {
            name: (*name).to_owned(),
            size: bytes.len() as u64,
            crc32: crc32fast::hash(bytes),
        })
        .collect();
    let (parts, len) = layout(&entries, modified).unwrap();
    let mut out = Vec::new();
    for part in parts {
        match part {
            Part::Bytes(bytes) => out.extend_from_slice(&bytes),
            Part::File(index) => out.extend_from_slice(files[index].1),
        }
    }
    assert_eq!(out.len() as u64, len);
    out
}

#[test]
fn a_zip_reader_reads_back_every_file() {
    let files: [(&str, &[u8]); 3] = [
        ("shot.png", b"\x89PNG\r\n\x1a\n first"),
        ("flow.webm", b"\x1a\x45\xdf\xa3 recording"),
        ("zrzut ekranu ąę.png", b""),
    ];
    let bytes = build(&files, Modified::from_unix(1_760_875_200));

    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    assert_eq!(zip.len(), 3);
    for (index, (name, content)) in files.iter().enumerate() {
        let mut file = zip.by_index(index).unwrap();
        assert_eq!(file.name().unwrap(), *name);
        assert_eq!(file.compression(), zip::CompressionMethod::Stored);
        assert_eq!(file.unix_mode(), Some(0o100_644));
        let mut read = Vec::new();
        // Checks the CRC as it reaches the end.
        file.read_to_end(&mut read).unwrap();
        assert_eq!(&read, content);
        let modified = file.last_modified().unwrap();
        let date = (modified.year(), modified.month(), modified.day());
        let time = (modified.hour(), modified.minute(), modified.second());
        assert_eq!((date, time), ((2025, 10, 19), (12, 0, 0)));
        let unix = file.extra_data_fields().find_map(|field| match field {
            zip::ExtraField::ExtendedTimestamp(t) => t.mod_time(),
            _ => None,
        });
        assert_eq!(unix, Some(1_760_875_200));
    }
}

#[test]
fn an_empty_demo_is_an_empty_zip() {
    let bytes = build(&[], Modified::from_unix(0));
    // The end record alone.
    assert_eq!(bytes.len(), 22);
    assert_eq!(zip::ZipArchive::new(Cursor::new(bytes)).unwrap().len(), 0);
}

#[test]
fn refuses_what_needs_zip64() {
    let entry = |size| Entry {
        name: "big.mp4".into(),
        size,
        crc32: 0,
    };
    let modified = Modified::from_unix(0);
    assert_eq!(layout(&[entry(1 << 32)], modified), Err(TooLarge));
    // All ones means "see zip64", so it's out too.
    let all_ones = u64::from(u32::MAX);
    assert_eq!(layout(&[entry(all_ones)], modified), Err(TooLarge));
    let empty = Entry {
        name: "a".into(),
        size: 0,
        crc32: 0,
    };
    let many = vec![empty; usize::from(u16::MAX)];
    assert_eq!(layout(&many, modified), Err(TooLarge));
    assert!(layout(&many[1..], modified).is_ok());
    let near = u64::from(u32::MAX) - 100;
    assert_eq!(layout(&[entry(near), entry(near)], modified), Err(TooLarge));
    assert!(layout(&[entry(100 << 20)], modified).is_ok());
}

#[test]
fn numbers_names_taken_already() {
    let names = unique_names([
        "shot.png", "shot.png", "Shot.PNG", "flow", "flow", ".hidden", ".hidden", "a.tar.gz",
        "a.tar.gz",
    ]);
    assert_eq!(
        names,
        [
            "shot.png",
            "shot (2).png",
            "Shot (3).PNG",
            "flow",
            "flow (2)",
            ".hidden",
            ".hidden (2)",
            "a.tar.gz",
            "a.tar (2).gz",
        ]
    );
}

#[test]
fn dos_time_reads_the_calendar() {
    let at = |secs| {
        let t = Modified::from_unix(secs);
        let date = (1980 + (t.date >> 9), (t.date >> 5) & 15, t.date & 31);
        let time = (t.time >> 11, (t.time >> 5) & 63, (t.time & 31) * 2);
        (date, time)
    };
    // 2024-02-29T23:59:59Z: a leap day, seconds rounded down to even.
    assert_eq!(at(1_709_251_199), ((2024, 2, 29), (23, 59, 58)));
    assert_eq!(at(315_532_800), ((1980, 1, 1), (0, 0, 0)));
    // Before 1980 is 1980.
    assert_eq!(at(0), ((1980, 1, 1), (0, 0, 0)));
    assert_eq!(at(4_354_819_200), ((2107, 12, 31), (23, 59, 58)));
    // Past 2038 the Unix time stops where a signed one ends.
    assert_eq!(Modified::from_unix(4_354_819_200).unix, i32::MAX as u32);
    assert_eq!(Modified::from_unix(1_709_251_199).unix, 1_709_251_199);
}
