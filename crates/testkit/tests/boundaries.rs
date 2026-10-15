//! The architecture's dependency rules, enforced on the crate graph:
//! - `core` has no I/O and knows no wire format: it depends on no workspace
//!   crate, `api` included, and on nothing outside the allowlist below.
//! - `api` depends on no workspace crate.
//! - Adapters (every other crate under `crates/` except the composition roots
//!   and this harness) depend on no workspace crate but `core` and `api`.

use std::process::Command;

use serde_json::Value;

/// Everything `core` may depend on. Adding to it is an architecture decision:
/// nothing here may do I/O, start a runtime or define a wire format. Wire
/// types and the domain to wire mapping live in `hub` and `cli`.
const CORE_ALLOWED: &[&str] = &["gix-imara-diff", "globset", "regex", "thiserror"];
const ADAPTER_ALLOWED_WORKSPACE_DEPS: &[&str] = &["mergegate-core", "mergegate-api"];
/// Crates allowed to depend on anything: the composition roots and the harness.
const UNRESTRICTED: &[&str] = &["cli", "hub", "testkit"];

struct Crate {
    dir: String,
    deps: Vec<Dep>,
}

struct Dep {
    name: String,
    workspace: bool,
}

fn workspace_crates() -> Vec<Crate> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run cargo metadata");
    assert!(output.status.success(), "{output:?}");
    let metadata: Value = serde_json::from_slice(&output.stdout).unwrap();
    metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|package| {
            let manifest = std::path::Path::new(package["manifest_path"].as_str().unwrap());
            let dir = manifest.parent().unwrap().file_name().unwrap();
            let deps = package["dependencies"]
                .as_array()
                .unwrap()
                .iter()
                // Normal dependencies only; dev-dependencies don't ship.
                .filter(|dep| dep["kind"].is_null())
                .map(|dep| Dep {
                    name: dep["name"].as_str().unwrap().to_owned(),
                    workspace: dep.get("path").is_some_and(|p| !p.is_null()),
                })
                .collect();
            Crate {
                dir: dir.to_str().unwrap().to_owned(),
                deps,
            }
        })
        .collect()
}

#[test]
fn crate_graph_follows_the_architecture() {
    let crates = workspace_crates();
    assert!(
        crates.iter().any(|c| c.dir == "testkit"),
        "metadata lists the workspace"
    );
    let mut violations = Vec::new();
    for krate in &crates {
        for dep in &krate.deps {
            let allowed = match krate.dir.as_str() {
                "core" => !dep.workspace && CORE_ALLOWED.contains(&dep.name.as_str()),
                "api" => !dep.workspace,
                dir if UNRESTRICTED.contains(&dir) => true,
                _ => !dep.workspace || ADAPTER_ALLOWED_WORKSPACE_DEPS.contains(&dep.name.as_str()),
            };
            if !allowed {
                violations.push(format!(
                    "crates/{} must not depend on {}",
                    krate.dir, dep.name
                ));
            }
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}
