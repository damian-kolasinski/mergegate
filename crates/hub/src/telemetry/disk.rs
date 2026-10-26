use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use api::diagnostics::Record;

pub(crate) const FILES: usize = 5;
pub(crate) const RETENTION: u64 = 7 * 24 * 60 * 60;
const DEFAULT_BYTES: u64 = 10 * 1024 * 1024;

pub(super) fn run(
    directory: PathBuf,
    receiver: Receiver<Record>,
    stopping: &AtomicBool,
    dropped: &AtomicU64,
) {
    let limit = std::env::var("MG_TELEMETRY_FILE_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|n| (1024..=100 * 1024 * 1024).contains(n))
        .unwrap_or(DEFAULT_BYTES);
    let mut disk = Disk { directory, limit };
    let mut pruned = super::now();
    let _ = disk.prune(pruned);
    while !stopping.load(Ordering::Relaxed) {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(record) => {
                if disk.write(&record).is_err() {
                    dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let now = super::now();
        if now.saturating_sub(pruned) >= 60 {
            let _ = disk.prune(now);
            pruned = now;
        }
    }
    for record in receiver.try_iter().take(256) {
        let _ = disk.write(&record);
    }
}

struct Disk {
    directory: PathBuf,
    limit: u64,
}

pub(crate) fn path(directory: &Path, index: usize) -> PathBuf {
    directory.join(format!("hub.{index}.jsonl"))
}

impl Disk {
    fn prune(&self, now: u64) -> io::Result<()> {
        let cutoff = now.saturating_sub(RETENTION);
        for index in 0..FILES {
            let path = path(&self.directory, index);
            let file = match File::open(&path) {
                Ok(file) => file,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            };
            let mut kept = Vec::new();
            let mut expired = false;
            let malformed = read(file, |record| {
                if record.timestamp < cutoff {
                    expired = true;
                } else {
                    kept.push(record);
                }
            })?;
            if !expired && !malformed {
                continue;
            }
            if kept.is_empty() {
                fs::remove_file(path)?;
            } else {
                let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)?;
                for record in kept {
                    serde_json::to_writer(&mut temporary, &record)?;
                    temporary.write_all(b"\n")?;
                }
                temporary.persist(path).map_err(|e| e.error)?;
            }
        }
        Ok(())
    }

    fn write(&mut self, record: &Record) -> io::Result<()> {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.directory)?;
        fs::set_permissions(&self.directory, fs::Permissions::from_mode(0o700))?;
        let mut bytes = serde_json::to_vec(record)?;
        bytes.push(b'\n');
        if bytes.len() as u64 > self.limit || bytes.len() > 4096 {
            return Err(io::Error::other("telemetry record exceeds file limit"));
        }
        let current = path(&self.directory, 0);
        if fs::metadata(&current).map_or(0, |m| m.len()) + bytes.len() as u64 + 1 > self.limit {
            let _ = fs::remove_file(path(&self.directory, FILES - 1));
            for index in (0..FILES - 1).rev() {
                match fs::rename(
                    path(&self.directory, index),
                    path(&self.directory, index + 1),
                ) {
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    result => result?,
                }
            }
        }
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .mode(0o600)
            .open(current)?;
        if file.metadata()?.len() > 0 {
            file.seek(SeekFrom::End(-1))?;
            let mut last = [0];
            file.read_exact(&mut last)?;
            // A killed writer may leave a partial line. Isolate it so the
            // next run's startup record remains independently readable.
            if last[0] != b'\n' {
                file.write_all(b"\n")?;
            }
        }
        file.write_all(&bytes)
    }
}

/// A bounded line read also tolerates a partial final write after a crash.
pub(crate) fn read(file: File, mut take: impl FnMut(Record)) -> io::Result<bool> {
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    let mut malformed = false;
    loop {
        line.clear();
        let n = std::io::Read::take(&mut reader, 4097).read_until(b'\n', &mut line)?;
        if n == 0 {
            return Ok(malformed);
        }
        if line.last() != Some(&b'\n') {
            malformed = true;
            // Discard the rest of an oversized or incomplete record.
            loop {
                let chunk = reader.fill_buf()?;
                if chunk.is_empty() {
                    return Ok(malformed);
                }
                let end = chunk.iter().position(|b| *b == b'\n');
                let consumed = end.map_or(chunk.len(), |i| i + 1);
                reader.consume(consumed);
                if end.is_some() {
                    break;
                }
            }
            continue;
        }
        match serde_json::from_slice(&line) {
            Ok(record) => take(record),
            Err(_) => malformed = true,
        }
    }
}

#[cfg(test)]
#[path = "disk_tests.rs"]
mod tests;
