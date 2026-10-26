use std::collections::BTreeMap;
use std::fs::FileTimes;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

fn record(sequence: u64) -> Record {
    Record {
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        run_id: "test".into(),
        version: "0.9.0+abcdef".into(),
        detail: Some("safe error detail".into()),
        sequence,
        level: "INFO".into(),
        target: "hub".into(),
        line: Some(1),
        fields: BTreeMap::new(),
        dropped: 0,
    }
}

#[test]
fn rotation_bounds_storage_and_preserves_the_newest_record() {
    let dir = tempfile::tempdir().unwrap();
    let mut disk = Disk {
        directory: dir.path().to_owned(),
        limit: 1024,
    };
    for n in 0..200 {
        disk.write(&record(n)).unwrap();
    }
    let entries = fs::read_dir(dir.path())
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), FILES);
    assert!(entries.iter().all(|e| e.metadata().unwrap().len() <= 1024));
    let mut newest = Vec::new();
    read(File::open(path(dir.path(), 0)).unwrap(), |r| newest.push(r)).unwrap();
    assert_eq!(newest.last().unwrap().sequence, 199);
    for entry in entries {
        assert_eq!(
            entry.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn retention_removes_old_files_even_without_new_records() {
    let dir = tempfile::tempdir().unwrap();
    let mut disk = Disk {
        directory: dir.path().to_owned(),
        limit: 1024,
    };
    let mut old = record(1);
    old.timestamp = 1;
    disk.write(&old).unwrap();
    let file = File::options()
        .write(true)
        .open(path(dir.path(), 0))
        .unwrap();
    file.set_times(FileTimes::new().set_modified(UNIX_EPOCH))
        .unwrap();
    disk.prune(super::super::now()).unwrap();
    assert!(!path(dir.path(), 0).exists());
}

#[test]
fn interrupted_and_oversized_records_do_not_hide_valid_evidence() {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(&vec![b'x'; 100_000]).unwrap();
    file.write_all(b"\n").unwrap();
    serde_json::to_writer(&mut file, &record(3)).unwrap();
    file.write_all(b"\n{partial").unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut found = Vec::new();
    read(file, |r| found.push(r)).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].sequence, 3);
}

#[test]
fn active_files_expire_old_records_without_losing_new_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let mut disk = Disk {
        directory: dir.path().to_owned(),
        limit: 4096,
    };
    let mut old = record(1);
    old.timestamp = 1;
    disk.write(&old).unwrap();
    disk.write(&record(2)).unwrap();
    disk.prune(super::super::now()).unwrap();
    let mut kept = Vec::new();
    read(File::open(path(dir.path(), 0)).unwrap(), |r| kept.push(r)).unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].sequence, 2);
}

#[test]
fn startup_after_a_truncated_write_remains_readable() {
    let dir = tempfile::tempdir().unwrap();
    let mut disk = Disk {
        directory: dir.path().to_owned(),
        limit: 4096,
    };
    disk.write(&record(1)).unwrap();
    OpenOptions::new()
        .append(true)
        .open(path(dir.path(), 0))
        .unwrap()
        .write_all(b"{interrupted")
        .unwrap();
    disk.write(&record(2)).unwrap();
    let mut kept = Vec::new();
    read(File::open(path(dir.path(), 0)).unwrap(), |r| kept.push(r)).unwrap();
    assert_eq!(kept.iter().map(|r| r.sequence).collect::<Vec<_>>(), [1, 2]);
}

#[test]
fn maintenance_drops_crash_fragments_even_without_valid_timestamps() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(path(dir.path(), 0), b"{partial private detail").unwrap();
    let disk = Disk {
        directory: dir.path().to_owned(),
        limit: 1024,
    };
    disk.prune(super::super::now()).unwrap();
    assert!(!path(dir.path(), 0).exists());
}
