use std::path::PathBuf;

use super::super::Spec;
use super::{exe, render};

fn spec(exe: &str) -> Spec {
    Spec {
        exe: PathBuf::from(exe),
        port: 7777,
        home: PathBuf::from("/home/op/.mergegate"),
        outcome: PathBuf::from("/home/op/.mergegate/hub.outcome"),
        log: PathBuf::from("/home/op/.mergegate/hub.log"),
        path_env: "/home/op/.cargo/bin:/usr/bin".into(),
    }
}

#[test]
fn the_unit_runs_the_hub_in_the_foreground_and_restarts_only_a_failed_one() {
    let unit = render(&spec("/home/op/.cargo/bin/mg"));
    for line in [
        "ExecStart=\"/home/op/.cargo/bin/mg\" \"--json\" \"hub\" \"start\" \"--foreground\" \"--port\" \"7777\"\n",
        "Environment=\"PATH=/home/op/.cargo/bin:/usr/bin\"\n",
        "WorkingDirectory=/home/op/.mergegate\n",
        "StandardOutput=append:/home/op/.mergegate/hub.outcome\n",
        "StandardError=append:/home/op/.mergegate/hub.log\n",
        "Restart=on-failure\n",
        "WantedBy=default.target\n",
    ] {
        assert!(unit.contains(line), "missing {line:?} in:\n{unit}");
    }
}

#[test]
fn specifiers_variables_and_quotes_stay_literal() {
    let odd = "/home/op/100% \"$HOME\"\\mg";
    let unit = render(&spec(odd));
    assert!(
        unit.contains("ExecStart=\"/home/op/100%% \\\"$$HOME\\\"\\\\mg\" "),
        "{unit}"
    );
    assert_eq!(exe(&unit), Some(PathBuf::from(odd)));
}

#[test]
fn a_unit_without_a_quoted_exec_start_has_no_binary() {
    assert_eq!(exe("[Service]\nExecStart=/usr/bin/mg hub start\n"), None);
    assert_eq!(exe("[Service]\n"), None);
}
