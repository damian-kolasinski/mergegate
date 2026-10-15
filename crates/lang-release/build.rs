//! Bakes in the index of the language packs released with this mg: the
//! `languages.json` that `langs/build.mjs` wrote, named by
//! `MG_LANGUAGES_INDEX`. Without it, as in dev builds, the index is empty.

use std::path::PathBuf;
use std::{env, fs};

fn main() {
    println!("cargo::rerun-if-env-changed=MG_LANGUAGES_INDEX");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let index = match env::var_os("MG_LANGUAGES_INDEX") {
        Some(path) => {
            println!("cargo::rerun-if-changed={}", PathBuf::from(&path).display());
            fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("can't read MG_LANGUAGES_INDEX {}: {e}", path.display()))
        }
        None => r#"{"packs":[]}"#.to_owned(),
    };
    fs::write(out.join("languages.json"), index).expect("write the index");
}
