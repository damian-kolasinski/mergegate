use std::io::{self, Write};
use std::path::Path;

use api::diagnostics::Snapshot;
use zip::write::SimpleFileOptions;

pub(super) fn write(path: &Path, snapshot: &Snapshot, report: Option<&str>) -> io::Result<()> {
    save(path, snapshot, report, false)
}

pub(super) fn replace(path: &Path, snapshot: &Snapshot, report: &str) -> io::Result<()> {
    save(path, snapshot, Some(report), true)
}

fn save(path: &Path, snapshot: &Snapshot, report: Option<&str>, replace: bool) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut zip = zip::ZipWriter::new(temporary.as_file_mut());
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o600);
        zip.start_file("diagnostics.json", options)?;
        serde_json::to_writer_pretty(&mut zip, snapshot)?;
        zip.start_file("README.txt", options)?;
        zip.write_all(b"mergegate diagnostics format 1\nContains operational metadata only; no database, environment, repository files or raw subprocess output.\nSelected report prose is sanitized heuristically; review it before sharing.\nAttach this ZIP manually to the issue after reviewing its contents. Public issue attachments are public.\n")?;
        if let Some(report) = report {
            zip.start_file("report.md", options)?;
            zip.write_all(report.as_bytes())?;
        }
        zip.finish()?;
    }
    // persist_noclobber publishes a complete ZIP and preserves existing files.
    if replace {
        temporary.persist(path).map_err(|e| e.error)?;
    } else {
        temporary.persist_noclobber(path).map_err(|e| e.error)?;
    }
    Ok(())
}
