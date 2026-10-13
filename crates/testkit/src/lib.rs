//! Test harness: runs the real `mg` binary against real hubs, each with its own
//! temporary `MERGEGATE_HOME`, and returns what `mg` printed and its exit code.
//!
//! Integration tests of the `mg` package pass the binary in:
//! `Env::new(env!("CARGO_BIN_EXE_mg"))`.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

/// One isolated mergegate installation. Dropping it stops its hub.
pub struct Env {
    home: TempDir,
    mg: PathBuf,
}

/// The result of one `mg` invocation.
#[derive(Debug)]
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    /// stdout parsed as the single JSON object `--json` promises.
    #[track_caller]
    pub fn json(&self) -> Value {
        serde_json::from_str(self.stdout.trim())
            .unwrap_or_else(|e| panic!("stdout is not one JSON object ({e}): {self:?}"))
    }

    #[track_caller]
    pub fn assert_code(self, code: i32) -> Self {
        assert_eq!(self.code, code, "unexpected exit code: {self:?}");
        self
    }
}

impl Env {
    pub fn new(mg: impl Into<PathBuf>) -> Self {
        Self {
            home: tempfile::Builder::new()
                .prefix("mg-home-")
                .tempdir()
                .expect("create MERGEGATE_HOME"),
            mg: mg.into(),
        }
    }

    pub fn home(&self) -> &Path {
        self.home.path()
    }

    pub fn mg(&self, args: &[&str]) -> Run {
        self.mg_with_env(args, &[])
    }

    pub fn mg_with_env(&self, args: &[&str], vars: &[(&str, &str)]) -> Run {
        let mut cmd = Command::new(&self.mg);
        cmd.args(args)
            .env("MERGEGATE_HOME", self.home())
            .env_remove("MG_JSON")
            .envs(vars.iter().copied());
        let output = cmd.output().expect("run mg");
        Run {
            code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// Starts this environment's hub on a free port and returns `mg`'s status.
    #[track_caller]
    pub fn start_hub(&self) -> Value {
        self.mg(&["--json", "hub", "start", "--port", "0"])
            .assert_code(0)
            .json()
    }

    /// The running hub's `host:port`, from its `hub.json`.
    #[track_caller]
    pub fn hub_addr(&self) -> String {
        let info = std::fs::read(self.home().join("hub.json")).expect("hub.json exists");
        let info: Value = serde_json::from_slice(&info).expect("hub.json parses");
        info["addr"].as_str().expect("addr").to_owned()
    }

    /// Sets the hub database's schema version, e.g. to one this build doesn't know.
    pub fn set_schema_version(&self, version: u32) {
        let conn = rusqlite::Connection::open(self.home().join("hub.db")).expect("open hub.db");
        conn.pragma_update(None, "user_version", version)
            .expect("set user_version");
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let info = self.home().join("hub.json");
        if !info.exists() || self.mg(&["hub", "stop"]).code == 0 {
            return;
        }
        // The hub didn't stop cleanly; don't leave it running after the test.
        if let Some(pid) = std::fs::read(&info)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v["pid"].as_u64())
        {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
    }
}

/// Sends a raw HTTP/1.1 request to `addr` and returns the status code and body.
/// Raw, so tests control every header, `Host` included.
pub fn http(addr: &str, request: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("connect to hub");
    stream.write_all(request.as_bytes()).expect("send request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    let status = response
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("malformed response: {response:?}"));
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_owned())
        .unwrap_or_default();
    (status, body)
}
