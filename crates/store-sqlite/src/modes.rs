//! The `old_mode` and `new_mode` columns `revision_files` and
//! `interdiff_files` share.

use mergegate_core::item::{ChangedFile, FileMode};
use mergegate_core::store::StoreError;
use rusqlite::Row;

/// The columns' values for `file`, old side first.
pub(crate) fn columns(file: &ChangedFile) -> [Option<&'static str>; 2] {
    [file.old_mode, file.new_mode].map(|mode| mode.map(FileMode::as_str))
}

/// Both sides' modes from `row`'s columns `first` and `first + 1`.
pub(crate) fn read(
    row: &Row,
    first: usize,
) -> rusqlite::Result<Result<[Option<FileMode>; 2], StoreError>> {
    let old: Option<String> = row.get(first)?;
    let new: Option<String> = row.get(first + 1)?;
    Ok(parse(old).and_then(|old| Ok([old, parse(new)?])))
}

fn parse(mode: Option<String>) -> Result<Option<FileMode>, StoreError> {
    mode.map(|m| {
        FileMode::parse(&m).ok_or_else(|| StoreError::new(format!("unknown file mode {m:?}")))
    })
    .transpose()
}
