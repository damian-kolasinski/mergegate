//! A stand-in for the `tailscale` CLI, so hubs listen on "the tailnet" on
//! loopback and tests choose which device each request comes from.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tempfile::TempDir;

/// The hub's machine is `hub`, owned by `owner@example.com`. Every request
/// over the tailnet comes from 127.0.0.1, so the device `whois` answers with
/// is the one [`FakeTailscale::call_from`] last chose, whatever the address.
pub struct FakeTailscale {
    dir: TempDir,
    peers: Vec<Peer>,
    running: bool,
}

struct Peer {
    name: String,
    login: String,
}

pub const HUB_OWNER: &str = "owner@example.com";

impl FakeTailscale {
    pub fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("mg-tailscale-")
            .tempdir()
            .expect("create the fake tailscale's directory");
        write_tailscale(&dir.path().join("bin"), dir.path());
        let fake = Self {
            dir,
            peers: Vec::new(),
            running: true,
        };
        fake.write_status();
        fake
    }

    /// Variables that make `mg`, and the hub it starts, use this fake and
    /// listen on "the tailnet" at a free loopback port.
    pub fn env(&self) -> Vec<(String, String)> {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![self.dir.path().join("bin")];
        paths.extend(std::env::split_paths(&path));
        let path = std::env::join_paths(paths).unwrap();
        vec![
            ("PATH".into(), path.to_str().unwrap().into()),
            ("MG_TAILNET_ADDR".into(), "127.0.0.1:0".into()),
            ("MG_TAILSCALE_CACHE_MS".into(), "0".into()),
            ("MG_TAILNET_RETRY_MS".into(), "100".into()),
            // A machine busy running tests can take seconds to start a shell.
            ("MG_TAILSCALE_TIMEOUT_MS".into(), "30000".into()),
        ]
    }

    /// Puts device `name`, owned by `login`, on the tailnet and makes it the
    /// caller of every request from here on.
    pub fn call_from(&mut self, name: &str, login: &str) {
        if !self.peers.iter().any(|p| p.name == name) {
            self.peers.push(Peer {
                name: name.into(),
                login: login.into(),
            });
        }
        self.write_status();
        let whois = json!({
            "Node": {
                "StableID": format!("n{name}"),
                "Name": format!("{name}.tail0.ts.net."),
                "Addresses": ["127.0.0.1/32"],
            },
            "UserProfile": { "ID": user_id(login), "LoginName": login },
        });
        self.write("whois.json", &whois);
    }

    /// Requests from here on come from an address Tailscale doesn't know.
    pub fn call_from_nobody(&self) {
        let _ = std::fs::remove_file(self.dir.path().join("whois.json"));
    }

    /// `false` stops Tailscale, as when the operator quits it; `true` starts it.
    pub fn set_running(&mut self, running: bool) {
        self.running = running;
        self.write_status();
    }

    fn write_status(&self) {
        let node = |name: &str, login: &str, ip: &str| {
            json!({
                "ID": format!("n{name}"),
                "HostName": name,
                "DNSName": format!("{name}.tail0.ts.net."),
                "UserID": user_id(login),
                "TailscaleIPs": [ip],
            })
        };
        let mut users = serde_json::Map::new();
        users.insert(
            user_id(HUB_OWNER).to_string(),
            json!({ "ID": user_id(HUB_OWNER), "LoginName": HUB_OWNER }),
        );
        let mut peers = serde_json::Map::new();
        for (i, peer) in self.peers.iter().enumerate() {
            users.insert(
                user_id(&peer.login).to_string(),
                json!({ "ID": user_id(&peer.login), "LoginName": peer.login }),
            );
            peers.insert(
                format!("nodekey:{}", peer.name),
                node(&peer.name, &peer.login, &format!("100.64.0.{}", i + 2)),
            );
        }
        let status = json!({
            "BackendState": if self.running { "Running" } else { "Stopped" },
            "Self": node("hub", HUB_OWNER, "100.64.0.1"),
            "Peer": peers,
            "User": users,
        });
        self.write("status.json", &status);
    }

    fn write(&self, file: &str, json: &Value) {
        std::fs::write(self.dir.path().join(file), json.to_string()).unwrap();
    }
}

impl Default for FakeTailscale {
    fn default() -> Self {
        Self::new()
    }
}

/// A stable id per login, as Tailscale gives each user one.
fn user_id(login: &str) -> u64 {
    login
        .bytes()
        .fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(u64::from(b)))
        % 1_000_000
}

/// `tailscale status --json` and `tailscale whois --json IP` print what the
/// fake last wrote; whois fails as the real one does for an unknown address.
fn write_tailscale(bin: &Path, dir: &Path) {
    std::fs::create_dir_all(bin).unwrap();
    let script = format!(
        "#!/bin/sh\n\
         case \"$1\" in\n\
         \x20 status) cat '{dir}/status.json' ;;\n\
         \x20 whois)\n\
         \x20   cat '{dir}/whois.json' 2>/dev/null && exit 0\n\
         \x20   echo \"peer not found\" >&2\n\
         \x20   exit 1 ;;\n\
         \x20 *) echo \"fake tailscale can't $*\" >&2; exit 1 ;;\n\
         esac\n",
        dir = dir.display()
    );
    let tailscale: PathBuf = bin.join("tailscale");
    std::fs::write(&tailscale, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tailscale, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}
