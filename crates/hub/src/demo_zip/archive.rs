//! The bytes of a zip around files the hub streams from disk: every header
//! is laid out up front, so the response knows its length before the first
//! file is read. Entries are stored, not deflated: screenshots and
//! recordings are compressed already.

/// A file going in. `crc32` is of its bytes, which go in the local header:
/// with it known, no entry needs a data descriptor, which some readers that
/// stream a zip refuse on a stored entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    pub name: String,
    pub size: u64,
    pub crc32: u32,
}

/// The archive in order: header bytes between the files, each file by its
/// index in the entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Part {
    Bytes(Vec<u8>),
    File(usize),
}

/// When the entries were modified. MS-DOS's fields, which every entry
/// carries, hold a local time, but the hub knows no time zone: they hold
/// UTC, and the Unix time in each entry's extended timestamp field is what
/// unzip, 7-Zip and libarchive go by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Modified {
    date: u16,
    time: u16,
    unix: u32,
}

/// Past what a zip without its zip64 extension holds: an offset or size
/// from 4 GiB less a byte, or 65535 entries, where those fields' all-ones
/// values send a reader looking for zip64 records. A demo is a few files of at most
/// 100 MiB each, so the hub refuses one this big rather than write zip64.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TooLarge;

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;
/// 2.0, the first version with stored entries in folders.
const NEEDED: u16 = 20;
/// Unix, so extracting keeps [`MODE`]; 3.0 of the spec.
const MADE_BY: u16 = (3 << 8) | 30;
/// A regular file, `rw-r--r--`, in the high half of the external attributes.
const MODE: u32 = 0o100_644 << 16;
/// Bit 11: names are UTF-8.
const UTF8: u16 = 1 << 11;
const STORED: u16 = 0;
/// The extended timestamp field ("UT"), with its modification time only:
/// tag, size, flags, then the time.
const TIMESTAMP: u16 = 0x5455;
const TIMESTAMP_LEN: u16 = 5;
const HAS_MODIFIED: u8 = 1;

/// The archive's parts and its total length, or [`TooLarge`].
pub(crate) fn layout(entries: &[Entry], modified: Modified) -> Result<(Vec<Part>, u64), TooLarge> {
    let count = fit16(entries.len() as u64)?;
    let mut extra = Vec::with_capacity(4 + usize::from(TIMESTAMP_LEN));
    put16(&mut extra, TIMESTAMP);
    put16(&mut extra, TIMESTAMP_LEN);
    extra.push(HAS_MODIFIED);
    put32(&mut extra, modified.unix);
    let extra_len = extra.len() as u16;
    let mut parts = Vec::with_capacity(entries.len() * 2 + 1);
    let mut central = Vec::new();
    let mut offset: u64 = 0;
    for (index, entry) in entries.iter().enumerate() {
        let name = entry.name.as_bytes();
        let name_len = fit16(name.len() as u64)?;
        let size = fit32(entry.size)?;
        let at = fit32(offset)?;

        let mut local = Vec::with_capacity(30 + name.len() + extra.len());
        put32(&mut local, LOCAL);
        put16(&mut local, NEEDED);
        put16(&mut local, UTF8);
        put16(&mut local, STORED);
        put16(&mut local, modified.time);
        put16(&mut local, modified.date);
        put32(&mut local, entry.crc32);
        put32(&mut local, size);
        put32(&mut local, size);
        put16(&mut local, name_len);
        put16(&mut local, extra_len);
        local.extend_from_slice(name);
        local.extend_from_slice(&extra);
        offset += local.len() as u64 + entry.size;
        parts.push(Part::Bytes(local));
        parts.push(Part::File(index));

        put32(&mut central, CENTRAL);
        put16(&mut central, MADE_BY);
        put16(&mut central, NEEDED);
        put16(&mut central, UTF8);
        put16(&mut central, STORED);
        put16(&mut central, modified.time);
        put16(&mut central, modified.date);
        put32(&mut central, entry.crc32);
        put32(&mut central, size);
        put32(&mut central, size);
        put16(&mut central, name_len);
        put16(&mut central, extra_len);
        put16(&mut central, 0); // comment
        put16(&mut central, 0); // disk
        put16(&mut central, 0); // internal attributes
        put32(&mut central, MODE);
        put32(&mut central, at);
        central.extend_from_slice(name);
        central.extend_from_slice(&extra);
    }
    let central_at = fit32(offset)?;
    let central_len = fit32(central.len() as u64)?;
    put32(&mut central, END);
    put16(&mut central, 0); // this disk
    put16(&mut central, 0); // the central directory's disk
    put16(&mut central, count);
    put16(&mut central, count);
    put32(&mut central, central_len);
    put32(&mut central, central_at);
    put16(&mut central, 0); // comment
    let len = offset + central.len() as u64;
    // The end record's own offsets fit, but a reader also needs the whole
    // archive's length to.
    fit32(len)?;
    parts.push(Part::Bytes(central));
    Ok((parts, len))
}

/// Names as a folder on disk can hold them all: a name already taken, by
/// letter case alone too as on macOS and Windows, gets ` (2)`, ` (3)`…
/// before its extension.
pub(crate) fn unique_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut taken = std::collections::HashSet::new();
    names
        .into_iter()
        .map(|name| {
            let (stem, extension) = match name.rfind('.') {
                Some(dot) if dot > 0 => name.split_at(dot),
                _ => (name, ""),
            };
            let mut candidate = name.to_owned();
            let mut n = 2;
            while !taken.insert(candidate.to_lowercase()) {
                candidate = format!("{stem} ({n}){extension}");
                n += 1;
            }
            candidate
        })
        .collect()
}

impl Modified {
    /// From seconds since the Unix epoch, each field clamped to what it
    /// holds: 1980 to 2107 for MS-DOS's, 2038 for the signed Unix time.
    pub(crate) fn from_unix(secs: u64) -> Self {
        let unix = secs.min(i32::MAX as u64) as u32;
        let days = secs / 86_400;
        let of_day = secs % 86_400;
        let (year, month, day) = civil(days);
        if year < 1980 {
            return Self {
                date: (1 << 5) | 1,
                time: 0,
                unix,
            };
        }
        if year > 2107 {
            return Self {
                date: (127 << 9) | (12 << 5) | 31,
                time: (23 << 11) | (59 << 5) | 29,
                unix,
            };
        }
        let (hour, minute, second) = (of_day / 3600, of_day / 60 % 60, of_day % 60);
        Self {
            date: (((year - 1980) << 9) | (month << 5) | day) as u16,
            time: ((hour << 11) | (minute << 5) | (second / 2)) as u16,
            unix,
        }
    }
}

/// Year, month and day of the day `days` after 1970-01-01, by Howard
/// Hinnant's `civil_from_days`, shifted so the arithmetic stays unsigned.
fn civil(days: u64) -> (u64, u64, u64) {
    // 1970-01-01 is day 719468 counted from 0000-03-01.
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

/// Below the all-ones value, which means "see zip64".
fn fit16(value: u64) -> Result<u16, TooLarge> {
    u16::try_from(value)
        .ok()
        .filter(|&v| v < u16::MAX)
        .ok_or(TooLarge)
}

fn fit32(value: u64) -> Result<u32, TooLarge> {
    u32::try_from(value)
        .ok()
        .filter(|&v| v < u32::MAX)
        .ok_or(TooLarge)
}

fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
