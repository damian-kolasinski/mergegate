use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use tar::{Builder, Header};

use super::*;

fn archive(dir: &Path, entries: &[(&str, EntryType)]) -> std::path::PathBuf {
    let path = dir.join("pack.tar.gz");
    let mut tar = Builder::new(GzEncoder::new(
        File::create(&path).unwrap(),
        Compression::fast(),
    ));
    for &(name, kind) in entries {
        let mut header = Header::new_gnu();
        header.set_entry_type(kind);
        header.set_size(if kind == EntryType::Regular { 2 } else { 0 });
        header.set_mode(0o644);
        if kind == EntryType::Symlink {
            header.set_link_name("/etc/passwd").unwrap();
        }
        // `append_data` would refuse the `..` names this test needs.
        header.as_gnu_mut().unwrap().name[..name.len()].copy_from_slice(name.as_bytes());
        header.set_cksum();
        let data: &[u8] = if kind == EntryType::Regular {
            b"{}"
        } else {
            b""
        };
        tar.append(&header, data).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap().flush().unwrap();
    path
}

#[test]
fn unpacks_plain_top_level_files() {
    let dir = tempfile::tempdir().unwrap();
    let into = dir.path().join("into");
    fs::create_dir(&into).unwrap();
    let from = archive(
        dir.path(),
        &[
            ("./", EntryType::Directory),
            ("./pack.json", EntryType::Regular),
            ("json.wasm", EntryType::Regular),
        ],
    );
    unpack(&from, &into).unwrap();
    assert_eq!(manifest(&into).unwrap(), "{}");
    assert!(into.join("json.wasm").is_file());
}

#[test]
fn refuses_anything_else() {
    for entries in [
        &[("../pack.json", EntryType::Regular)][..],
        &[("sub/pack.json", EntryType::Regular)],
        &[("/pack.json", EntryType::Regular)],
        &[("sub", EntryType::Directory)],
        &[("pack.json", EntryType::Symlink)],
        &[("._pack.json", EntryType::Regular)],
        &[
            ("pack.json", EntryType::Regular),
            ("./pack.json", EntryType::Regular),
        ],
        &[("json.wasm", EntryType::Regular)],
    ] {
        let dir = tempfile::tempdir().unwrap();
        let into = dir.path().join("into");
        fs::create_dir(&into).unwrap();
        let from = archive(dir.path(), entries);
        assert!(unpack(&from, &into).is_err(), "{entries:?}");
        assert!(!dir.path().join("pack.json").exists());
    }
}

#[test]
fn refuses_a_file_that_is_not_an_archive() {
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("pack.tar.gz");
    fs::write(&from, "not gzip").unwrap();
    assert!(unpack(&from, dir.path()).is_err());
}
