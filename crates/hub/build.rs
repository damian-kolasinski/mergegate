//! Builds the UI in `ui/` with pnpm and embeds it: `ui_assets.rs` in OUT_DIR
//! lists every built file with its ETag. Without pnpm, or when the UI build
//! fails, the hub gets a stub page instead, so a Rust-only toolchain still
//! builds mg; MG_REQUIRE_UI=1 turns that into a build error.

use std::fmt::Write as _;
use std::fs::File;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Entries of `ui/` the bundle isn't built from: installs, generated
/// bindings, build and test output.
const UNTRACKED: &[&str] = &[
    "node_modules",
    "dist",
    "api",
    "tests",
    "test-results",
    "playwright-report",
    ".build.lock",
];

fn main() {
    let manifest_dir = PathBuf::from(env("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(env("OUT_DIR"));
    let ui = manifest_dir.join("../../ui");
    let site = out_dir.join("ui");
    let required = std::env::var("MG_REQUIRE_UI").is_ok_and(|v| v == "1");

    let built = match build(&ui, &site) {
        Ok(()) => true,
        Err(reason) if required => panic!("building the UI failed: {reason}"),
        Err(reason) => {
            // Only the stub depends on MG_REQUIRE_UI. Watching it after a
            // real build would rebuild the hub whenever a command sets it
            // differently, as the Playwright setup does.
            println!("cargo::rerun-if-env-changed=MG_REQUIRE_UI");
            println!(
                "cargo::warning=the hub gets a stub UI: {reason}. \
                 Set MG_REQUIRE_UI=1 to make this an error."
            );
            stub(&site);
            false
        }
    };
    write_table(&site, built, &out_dir.join("ui_assets.rs"));
}

fn build(ui: &Path, site: &Path) -> Result<(), String> {
    if !ui.join("package.json").is_file() {
        return Err(format!("no UI sources at {}", ui.display()));
    }
    for entry in read_dir(ui) {
        let name = entry.file_name();
        if !UNTRACKED.iter().any(|u| name == *u) {
            println!("cargo::rerun-if-changed={}", entry.path().display());
        }
    }

    // Builds of other profiles or targets run this script at the same time,
    // and two pnpm installs into one node_modules corrupt it.
    let lock = File::create(ui.join(".build.lock")).map_err(|e| format!("ui/.build.lock: {e}"))?;
    lock.lock()
        .map_err(|e| format!("locking ui/.build.lock: {e}"))?;

    pnpm(ui, &["install", "--frozen-lockfile", "--prefer-offline"])?;
    let _ = std::fs::remove_dir_all(site);
    let outdir = site.to_str().ok_or("OUT_DIR is not UTF-8")?;
    pnpm(ui, &["build", "--outdir", outdir])
}

fn pnpm(ui: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("pnpm")
        .args(args)
        .current_dir(ui)
        // Without a terminal, pnpm aborts instead of asking whether to
        // reinstall node_modules made by another pnpm version.
        .env("npm_config_confirm_modules_purge", "false")
        .output()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => "pnpm is not installed".to_owned(),
            _ => format!("running pnpm: {e}"),
        })?;
    if output.status.success() {
        return Ok(());
    }
    let mut log = String::from_utf8_lossy(&output.stdout).into_owned();
    log.push_str(&String::from_utf8_lossy(&output.stderr));
    let tail: Vec<&str> = log.lines().rev().take(20).collect();
    let tail: Vec<&str> = tail.into_iter().rev().collect();
    Err(format!(
        "`pnpm {}` failed ({}):\n{}",
        args.join(" "),
        output.status,
        tail.join("\n")
    ))
}

fn stub(site: &Path) {
    let _ = std::fs::remove_dir_all(site);
    std::fs::create_dir_all(site).expect("creating the stub UI directory");
    std::fs::write(
        site.join("index.html"),
        "<!doctype html>\n<meta charset=\"utf-8\">\n<title>mergegate</title>\n\
         <h1>This mg was built without its UI</h1>\n\
         <p>Install pnpm, then rebuild mg with MG_REQUIRE_UI=1 set. \
         <code>mg doctor</code> shows the same.</p>\n",
    )
    .expect("writing the stub UI");
}

/// Writes the table `src/ui.rs` includes: `BUILT`, and `ASSETS` with each
/// file's path relative to `site`.
fn write_table(site: &Path, built: bool, table: &Path) {
    let mut files = Vec::new();
    collect(site, &mut files);
    files.sort();

    let mut rust = format!("const BUILT: bool = {built};\n\n");
    rust.push_str("static ASSETS: &[Asset] = &[\n");
    for file in files {
        let path = file
            .strip_prefix(site)
            .expect("collected under the site")
            .components()
            .map(|c| c.as_os_str().to_str().expect("UI file names are UTF-8"))
            .collect::<Vec<_>>()
            .join("/");
        let body = std::fs::read(&file).expect("reading a built UI file");
        let mut hasher = DefaultHasher::new();
        body.hash(&mut hasher);
        let etag = format!("\"{:016x}\"", hasher.finish());
        let source = file.to_str().expect("OUT_DIR is UTF-8");
        writeln!(
            rust,
            "    Asset {{ path: {path:?}, etag: {etag:?}, body: include_bytes!({source:?}) }},"
        )
        .expect("writing to a String");
    }
    rust.push_str("];\n");
    std::fs::write(table, rust).expect("writing ui_assets.rs");
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in read_dir(dir) {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, files);
        } else {
            files.push(path);
        }
    }
}

fn read_dir(dir: &Path) -> impl Iterator<Item = std::fs::DirEntry> {
    std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|entry| entry.expect("reading a directory entry"))
}

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("cargo sets {name}"))
}
