//! Derives `API_VERSION` from this crate's sources, so it changes whenever a
//! wire type does and never needs bumping by hand: a hub left running across
//! an upgrade that changed the wire is refused, not sent fields it drops.
//! Comments and whitespace are left out, so rewording a doc or reformatting
//! asks no one to restart their hub.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let src = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo")).join("src");
    println!("cargo::rerun-if-changed=src");
    let mut files = Vec::new();
    collect(&src, &mut files);
    files.sort();

    let mut hash = Fnv::default();
    for file in files {
        let relative = file.strip_prefix(&src).expect("collected under src");
        hash.write(
            relative
                .to_str()
                .expect("source paths are UTF-8")
                .as_bytes(),
        );
        let text = std::fs::read_to_string(&file).expect("reading an api source");
        for line in text.lines() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            let code: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            hash.write(code.as_bytes());
        }
    }
    let version = hash.finish();

    let mut rust = String::new();
    writeln!(rust, "const FINGERPRINT: u32 = {version:#010x};").expect("writing to a String");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("set by cargo"));
    std::fs::write(out.join("api_version.rs"), rust).expect("writing api_version.rs");
}

/// Every `.rs` file under `dir` but the unit tests beside the code, which
/// change no wire type.
fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("reading the api sources") {
        let path = entry.expect("reading the api sources").path();
        if path.is_dir() {
            collect(&path, files);
        } else if path.extension().is_some_and(|e| e == "rs")
            && !path.to_str().is_some_and(|p| p.ends_with("_tests.rs"))
        {
            files.push(path);
        }
    }
}

/// FNV-1a, folded to 32 bits. Not std's hasher: its output may change between
/// Rust releases, and a CLI and hub built from the same sources by different
/// toolchains, as on two machines of a tailnet, must agree.
struct Fnv(u64);

impl Default for Fnv {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv {
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
        // Separates one write from the next, so moving code across a line
        // or file boundary still changes the hash.
        self.0 ^= 0xff;
        self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
    }

    fn finish(&self) -> u32 {
        #[allow(clippy::cast_possible_truncation)]
        let folded = (self.0 ^ (self.0 >> 32)) as u32;
        folded
    }
}
