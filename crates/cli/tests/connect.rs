//! F2, multi-machine: a hub listens on the tailnet, a worker machine's `mg`
//! connects to it, and the hub lets in the owner's devices and the ones the
//! operator allowed. Both machines are homes on this one, and a fake
//! `tailscale` says which device each request over "the tailnet" comes from.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};
use testkit::{Env, FakeTailscale, HUB_OWNER, TestRepo, http_reply};

struct Tailnet {
    tailscale: FakeTailscale,
    hub: Env,
    worker: Env,
    /// The hub's tailnet URL, as `mg hub set listen tailnet` printed it.
    url: String,
}

impl Tailnet {
    /// A hub listening on the tailnet, and a worker that hasn't connected.
    fn new() -> Self {
        let tailscale = FakeTailscale::new();
        let hub = Env::new(env!("CARGO_BIN_EXE_mg")).with_vars(tailscale.env());
        hub.start_hub();
        let set = hub
            .mg(&["--json", "hub", "set", "listen", "tailnet"])
            .assert_code(0)
            .json();
        assert_eq!(set["listen"], "tailnet");
        let url = set["tailnet"]["url"]
            .as_str()
            .expect("tailnet url")
            .to_owned();
        let worker = Env::new(env!("CARGO_BIN_EXE_mg"));
        Self {
            tailscale,
            hub,
            worker,
            url,
        }
    }

    /// The tailnet listener's `host:port`.
    fn addr(&self) -> &str {
        self.url.strip_prefix("http://").unwrap()
    }

    fn connect(&self) -> testkit::Run {
        self.worker.mg(&["--json", "connect", &self.url])
    }

    /// A raw request over the tailnet, addressed as `host`.
    fn request(&self, method: &str, path: &str, host: &str, body: &Value) -> testkit::Reply {
        let body = body.to_string();
        http_reply(
            self.addr(),
            &format!(
                "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\
                 {}: mg\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                api::CLIENT_HEADER,
                body.len(),
            ),
        )
    }
}

#[track_caller]
fn error(run: &testkit::Run) -> Value {
    assert_ne!(run.code, 0, "{run:?}");
    run.json()["error"].clone()
}

/// The hub's settings, as the UI reads them.
fn settings(hub: &Env) -> Value {
    let addr = hub.hub_addr();
    let reply = http_reply(
        &addr,
        &format!("GET /api/hub/settings HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}

#[test]
fn f2_listen_tailnet_prints_where_to_connect() {
    let tailscale = FakeTailscale::new();
    let hub = Env::new(env!("CARGO_BIN_EXE_mg")).with_vars(tailscale.env());
    hub.start_hub();
    let out = hub
        .mg(&["hub", "set", "listen", "tailnet"])
        .assert_code(0)
        .stdout;
    let url = out
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("listen tailnet: listening at "))
        .unwrap_or_else(|| panic!("{out}"));
    assert!(
        out.contains(&format!("on another machine: mg connect {url}")),
        "{out}"
    );

    // The setting outlives the hub.
    hub.mg(&["hub", "stop"]).assert_code(0);
    hub.start_hub();
    let settings = settings(&hub);
    assert_eq!(settings["listen"], "tailnet");
    assert!(settings["tailnet"]["url"].is_string(), "{settings}");
}

#[test]
fn f2_the_owners_device_connects_on_its_own() {
    let mut net = Tailnet::new();
    net.tailscale.call_from("laptop", HUB_OWNER);
    let connected = net.connect().assert_code(0).json();
    assert_eq!(connected["url"], net.url.as_str());
    assert_eq!(connected["device"]["name"], "laptop");
    assert_eq!(connected["device"]["trust"], "owner");

    let text = net.worker.mg(&["connect", &net.url]).assert_code(0).stdout;
    assert_eq!(
        text.trim(),
        format!(
            "connected to the hub at {} as laptop (the hub owner's device)",
            net.url
        )
    );
    let status = net.worker.mg(&["--json", "hub", "status"]).json();
    assert_eq!(status["connected"], net.url.as_str());
    let doctor = net.worker.mg(&["doctor"]).stdout;
    assert!(
        doctor.contains(&format!(
            "connected to {} as laptop, the hub owner's device",
            net.url
        )),
        "{doctor}"
    );

    let devices = net
        .hub
        .mg(&["--json", "hub", "devices"])
        .assert_code(0)
        .json();
    assert_eq!(
        devices["devices"],
        json!([
            { "name": "hub", "owner": HUB_OWNER, "standing": "owner", "on_tailnet": true },
            { "name": "laptop", "owner": HUB_OWNER, "standing": "owner", "on_tailnet": true },
        ])
    );

    // Back to this machine's own hub.
    net.worker.mg(&["connect", "--local"]).assert_code(0);
    let status = net.worker.mg(&["--json", "hub", "status"]).json();
    assert_eq!(status["connected"], Value::Null);
}

#[test]
fn f2_another_users_device_waits_for_the_operator() {
    let mut net = Tailnet::new();
    net.tailscale.call_from("ci", "robot@example.com");
    let refused = net.connect();
    let refusal = error(&refused);
    assert_eq!(refusal["code"], "device_not_allowed", "{refused:?}");
    assert_eq!(refusal["hint"], "on the hub: mg hub allow ci");
    assert!(!net.worker.home().join("connect.json").exists());

    let devices = net.hub.mg(&["hub", "devices"]).assert_code(0).stdout;
    assert!(
        devices.contains("asked") && devices.contains("ci"),
        "{devices}"
    );

    let allowed = net.hub.mg(&["hub", "allow", "ci"]).assert_code(0).stdout;
    assert_eq!(allowed.trim(), "allowed ci (robot@example.com)");
    let connected = net.connect().assert_code(0).json();
    assert_eq!(connected["device"]["trust"], "allowed");

    // A device that isn't on the tailnet can't be decided on.
    let unknown = net.hub.mg(&["--json", "hub", "allow", "nowhere"]);
    assert_eq!(error(&unknown)["code"], "unknown_device", "{unknown:?}");
}

#[test]
fn f2_a_denial_outweighs_ownership() {
    let mut net = Tailnet::new();
    net.tailscale.call_from("laptop", HUB_OWNER);
    net.connect().assert_code(0);
    net.hub.mg(&["hub", "deny", "laptop"]).assert_code(0);
    let refused = net.worker.mg(&["--json", "inbox"]);
    assert_eq!(error(&refused)["code"], "device_denied", "{refused:?}");

    net.hub.mg(&["hub", "allow", "laptop"]).assert_code(0);
    net.worker.mg(&["--json", "inbox"]).assert_code(0);
}

#[test]
fn f2_an_unknown_address_is_refused() {
    let net = Tailnet::new();
    net.tailscale.call_from_nobody();
    let reply = net.request("GET", "/api/whoami", net.addr(), &Value::Null);
    assert_eq!(reply.status, 403, "{}", reply.body);
    assert!(reply.body.contains("unknown_caller"), "{}", reply.body);
}

#[test]
fn f2_a_worker_submits_and_waits_over_the_tailnet() {
    let mut net = Tailnet::new();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    net.hub
        .mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);

    net.tailscale.call_from("laptop", HUB_OWNER);
    net.connect().assert_code(0);
    let clone = repo.sibling("laptop-clone");
    testkit::git(
        repo.root().parent().unwrap(),
        [
            "clone",
            "--quiet",
            repo.root().to_str().unwrap(),
            clone.to_str().unwrap(),
        ],
    );
    let init = net
        .worker
        .mg_at(&clone, &["--json", "init", "--project", "app"])
        .assert_code(0)
        .json();
    assert_eq!(init["url"], format!("{}/git/app", net.url).as_str());
    testkit::git(&clone, ["switch", "--quiet", "-c", "feature"]);
    testkit::commit_files(&clone, "Add login", &[("src/login.rs", "fn login() {}\n")]);

    // A wake command would run on the hub's machine, not this one.
    let refused = net
        .worker
        .mg_at(&clone, &["--json", "submit", "--wake", "true"]);
    assert_eq!(error(&refused)["code"], "wake_remote", "{refused:?}");

    let submitted = net
        .worker
        .mg_at(&clone, &["--json", "submit"])
        .assert_code(0)
        .json();
    assert_eq!(submitted["item"]["id"], "app#1");
    let inbox = net.hub.mg(&["--json", "inbox"]).assert_code(0).json();
    assert!(inbox.to_string().contains("\"app#1\""), "{inbox}");

    let wait = wait(&net.worker, &clone);
    std::thread::sleep(Duration::from_millis(500));
    review(&net.hub, "Looks good from here.");
    let output = wait.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("Looks good from here."));
}

#[test]
fn f2_the_hub_refuses_a_remote_wake_and_clears_a_local_one() {
    let mut net = Tailnet::new();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    net.hub
        .mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    net.hub
        .mg_at(repo.root(), &["--json", "init"])
        .assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Add login", &[("src/login.rs", "fn login() {}\n")]);
    let log = net.hub.home().join("woken.log");
    let wake = format!("echo woken >> {}", log.display());
    net.hub
        .mg_at(repo.root(), &["submit", "--wake", &wake])
        .assert_code(0);
    let head = repo.rev("HEAD");

    net.tailscale.call_from("laptop", HUB_OWNER);
    let submit = |wake: Option<&str>| {
        let mut body = json!({ "project": "app", "branch": "feature", "head": head });
        if let Some(wake) = wake {
            body["wake"] = wake.into();
        }
        net.request("POST", "/api/items", net.addr(), &body)
    };
    let refused = submit(Some("true"));
    assert_eq!(refused.status, 403, "{}", refused.body);
    assert!(refused.body.contains("wake_remote"), "{}", refused.body);

    // The item moved to a machine its wake can't reach, so no wake is left.
    let moved = submit(None);
    assert!(moved.status < 300, "{}", moved.body);
    review(&net.hub, "Nobody to wake here.");
    std::thread::sleep(Duration::from_millis(500));
    assert!(!log.exists(), "the local wake ran");
}

#[test]
fn f2_the_ui_answers_over_the_tailnet_when_addressed_as_the_hub() {
    let mut net = Tailnet::new();
    net.tailscale.call_from("laptop", HUB_OWNER);
    let page = net.request("GET", "/", net.addr(), &Value::Null);
    assert_eq!(page.status, 200, "{}", page.body);

    let port = net.addr().rsplit_once(':').unwrap().1;
    let renamed = net.request("GET", "/", &format!("hub:{port}"), &Value::Null);
    assert_eq!(renamed.status, 200, "{}", renamed.body);

    let rebound = net.request("GET", "/", &format!("evil.example:{port}"), &Value::Null);
    assert_eq!(rebound.status, 403, "{}", rebound.body);
}

#[test]
fn f2_listen_local_closes_the_tailnet() {
    let mut net = Tailnet::new();
    net.tailscale.call_from("laptop", HUB_OWNER);
    net.connect().assert_code(0);
    let set = net
        .hub
        .mg(&["hub", "set", "listen", "local"])
        .assert_code(0)
        .stdout;
    assert_eq!(set.trim(), "listen local");
    let unreachable = net.worker.mg(&["--json", "inbox"]);
    assert_eq!(
        error(&unreachable)["code"],
        "hub_unavailable",
        "{unreachable:?}"
    );
}

#[test]
fn f2_the_hub_keeps_trying_while_tailscale_is_down() {
    let mut net = Tailnet::new();
    net.hub.mg(&["hub", "stop"]).assert_code(0);
    net.tailscale.set_running(false);
    net.hub.start_hub();
    let doctor = net.hub.mg(&["doctor"]).stdout;
    assert!(
        doctor.contains("not listening on the tailnet yet"),
        "{doctor}"
    );

    net.tailscale.set_running(true);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let settings = settings(&net.hub);
        if settings["tailnet"]["url"].is_string() {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "{settings}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn f2_doctor_notices_tailscale_stopping_under_the_listener() {
    let mut net = Tailnet::new();
    net.tailscale.set_running(false);
    let doctor = net.hub.mg(&["doctor"]).stdout;
    assert!(
        doctor.contains(&format!(
            "listening on the tailnet at {}, but Tailscale isn't connected",
            net.url
        )),
        "{doctor}"
    );
}

#[test]
fn f2_a_denial_ends_a_wait_in_progress() {
    let (mut net, _repo) = with_an_item();
    net.tailscale.call_from("laptop", HUB_OWNER);
    net.connect().assert_code(0);
    let wait = waiting(&net.worker);
    std::thread::sleep(Duration::from_millis(500));
    net.hub.mg(&["hub", "deny", "laptop"]).assert_code(0);
    let refused = finished(wait);
    assert_eq!(refused["error"]["code"], "device_denied", "{refused}");
}

#[test]
fn f2_listen_local_ends_a_wait_in_progress() {
    let (mut net, _repo) = with_an_item();
    net.tailscale.call_from("laptop", HUB_OWNER);
    net.connect().assert_code(0);
    let wait = waiting(&net.worker);
    std::thread::sleep(Duration::from_millis(500));
    net.hub
        .mg(&["hub", "set", "listen", "local"])
        .assert_code(0);
    let refused = finished(wait);
    assert_eq!(
        refused["error"]["message"], "the hub no longer listens on the tailnet",
        "{refused}"
    );
}

#[test]
fn f2_a_connected_mg_sends_the_hub_no_local_paths() {
    let mut net = Tailnet::new();
    net.tailscale.call_from("laptop", HUB_OWNER);
    net.connect().assert_code(0);
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    let add = net.worker.mg_at(repo.root(), &["--json", "project", "add"]);
    assert_eq!(error(&add)["code"], "path_remote", "{add:?}");
    let packs = repo.root().to_str().unwrap();
    let lang = net.worker.mg(&["--json", "lang", "add", "--from", packs]);
    assert_eq!(error(&lang)["code"], "path_remote", "{lang:?}");
}

/// A hub listening on the tailnet with `app#1` submitted on its own machine.
fn with_an_item() -> (Tailnet, TestRepo) {
    let net = Tailnet::new();
    let repo = TestRepo::new("app");
    repo.commit_files("Base", &[("README", "title\n")]);
    net.hub
        .mg_at(repo.root(), &["--json", "project", "add"])
        .assert_code(0);
    net.hub
        .mg_at(repo.root(), &["--json", "init"])
        .assert_code(0);
    repo.git(["switch", "--quiet", "-c", "feature"]);
    repo.commit_files("Add login", &[("src/login.rs", "fn login() {}\n")]);
    net.hub.mg_at(repo.root(), &["submit"]).assert_code(0);
    (net, repo)
}

/// `mg wait app#1` on the worker, answering in JSON.
fn waiting(worker: &Env) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_mg"))
        .args(["--json", "wait", "app#1", "--timeout", "30"])
        .current_dir(worker.home())
        .env("MERGEGATE_HOME", worker.home())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
}

/// What a wait printed, once it ended well before its own timeout.
fn finished(wait: std::process::Child) -> Value {
    let started = std::time::Instant::now();
    let output = wait.wait_with_output().unwrap();
    assert!(started.elapsed() < Duration::from_secs(15), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("{e}: {output:?}"))
}

fn review(hub: &Env, summary: &str) {
    let addr = hub.hub_addr();
    let body = json!({ "revision": 1, "verdict": "comment", "summary": summary }).to_string();
    let reply = http_reply(
        &addr,
        &format!(
            "POST /api/items/app%231/reviews HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\
             {}: ui\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            api::CLIENT_HEADER,
            body.len(),
        ),
    );
    assert!(reply.status < 300, "{}", reply.body);
}

fn wait(worker: &Env, dir: &PathBuf) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_mg"))
        .args(["wait", "app#1", "--timeout", "30"])
        .current_dir(dir)
        .env("MERGEGATE_HOME", worker.home())
        .env_remove("MG_JSON")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap()
}
