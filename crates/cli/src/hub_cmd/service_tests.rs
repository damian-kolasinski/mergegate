use std::path::PathBuf;

use super::Spec;

#[test]
fn a_unit_runs_this_mg_as_the_foreground_hub_on_the_given_port() {
    let spec = Spec {
        exe: PathBuf::from("/opt/mg"),
        port: 0,
        home: PathBuf::from("/h"),
        outcome: PathBuf::from("/h/hub.outcome"),
        log: PathBuf::from("/h/hub.log"),
        path_env: String::new(),
    };
    assert_eq!(
        spec.args(),
        [
            "/opt/mg",
            "--json",
            "hub",
            "start",
            "--foreground",
            "--port",
            "0"
        ]
    );
}
