use std::path::PathBuf;

use super::{Role, Spec};

#[test]
fn a_unit_runs_this_mg_with_its_arguments() {
    let spec = Spec {
        role: Role::Hub,
        exe: PathBuf::from("/opt/mg"),
        args: vec!["--json".into(), "hub".into(), "start".into()],
        home: PathBuf::from("/h"),
        outcome: PathBuf::from("/h/hub.outcome"),
        log: PathBuf::from("/h/hub.log"),
        path_env: String::new(),
    };
    assert_eq!(spec.command(), ["/opt/mg", "--json", "hub", "start"]);
}
