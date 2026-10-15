use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use serde_json::Value;
use tempfile::TempDir;

/// One isolated mergegate installation. Dropping it stops its hub.
pub struct Env {
    home: TempDir,
    /// `HOME` for every `mg`, so nothing reads the user's own, such as the
    /// skills their agents load.
    user: TempDir,
    mg: PathBuf,
    /// Set for every `mg`, and so for the hub it starts.
    vars: Vec<(String, String)>,
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
            user: tempfile::Builder::new()
                .prefix("mg-user-")
                .tempdir()
                .expect("create HOME"),
            mg: mg.into(),
            vars: Vec::new(),
        }
    }

    /// Sets `vars` for every `mg` run from here on, as [`FakeGithub::env`]
    /// gives them.
    ///
    /// [`FakeGithub::env`]: crate::FakeGithub::env
    pub fn with_vars(mut self, vars: Vec<(String, String)>) -> Self {
        self.vars = vars;
        self
    }

    pub fn home(&self) -> &Path {
        self.home.path()
    }

    pub fn mg(&self, args: &[&str]) -> Run {
        self.mg_with_env(args, &[])
    }

    pub fn mg_with_env(&self, args: &[&str], vars: &[(&str, &str)]) -> Run {
        let mut cmd = self.command(args);
        cmd.envs(vars.iter().copied());
        Self::run(cmd)
    }

    /// Runs `mg` with `dir` as its working directory, as an agent in that
    /// repo would.
    pub fn mg_at(&self, dir: &Path, args: &[&str]) -> Run {
        let mut cmd = self.command(args);
        cmd.current_dir(dir);
        Self::run(cmd)
    }

    /// Starts `mg` without waiting for it, its stdout piped, for a command
    /// that runs until stopped such as `mg events --follow`. The caller
    /// kills it.
    pub fn spawn(&self, args: &[&str]) -> Child {
        self.command(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start mg")
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.mg);
        cmd.args(args)
            .env("MERGEGATE_HOME", self.home())
            .env("HOME", self.user.path())
            .env_remove("MG_JSON")
            .env_remove("MG_UI_DIR")
            .env_remove("MG_LANGUAGES_INDEX")
            .env_remove("MG_LANGUAGES_URL")
            .envs(self.vars.iter().map(|(k, v)| (k, v)));
        cmd
    }

    fn run(mut cmd: Command) -> Run {
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
