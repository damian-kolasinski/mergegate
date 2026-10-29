//! Writing and removing a skill folder, so that a reader never sees half of
//! one.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Writes `files`, by path relative to `path`, into a folder beside `path`
/// and swaps it in for whatever `path` was: a folder, a file, or a link,
/// whose target stays. Files an older folder had and `files` lacks go.
pub fn write(path: &Path, files: &[(&str, &str)]) -> io::Result<()> {
    let parent = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new()
        .prefix(".mergegate-skill-")
        .tempdir_in(parent)?;
    // A temporary folder is private; a skill folder is read like any other.
    fs::set_permissions(staging.path(), fs::Permissions::from_mode(0o755))?;
    for (file, contents) in files {
        let target = staging.path().join(file);
        if let Some(dir) = target.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&target, contents)?;
    }
    remove(path)?;
    fs::rename(staging.keep(), path)
}

/// Removes `path`, a folder, file or link (not its target), if it is there.
pub fn remove(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
