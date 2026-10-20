use std::path::{Path, PathBuf};

use super::super::{Role, Spec};
use super::{Session, exe, path, render};

fn spec(exe: &str) -> Spec {
    Spec {
        role: Role::Hub,
        exe: PathBuf::from(exe),
        args: ["--json", "hub", "start", "--foreground", "--port", "7777"]
            .map(String::from)
            .to_vec(),
        home: PathBuf::from("/Users/op/.mergegate"),
        outcome: PathBuf::from("/Users/op/.mergegate/hub.outcome"),
        log: PathBuf::from("/Users/op/.mergegate/hub.log"),
        path_env: "/opt/homebrew/bin:/usr/bin:/bin".into(),
    }
}

#[test]
fn the_agent_lives_in_the_users_launch_agents() {
    assert_eq!(
        path(Role::Hub, Path::new("/Users/op")),
        Path::new("/Users/op/Library/LaunchAgents/dev.mergegate.hub.plist")
    );
}

#[test]
fn the_plist_runs_the_hub_in_the_foreground_and_restarts_only_a_failed_one() {
    let plist = render(&spec("/opt/homebrew/bin/mg"), Session::Gui);
    for line in [
        "<string>dev.mergegate.hub</string>",
        "\t\t<string>/opt/homebrew/bin/mg</string>\n\t\t<string>--json</string>\n\t\t<string>hub</string>\n\t\t<string>start</string>\n\t\t<string>--foreground</string>\n\t\t<string>--port</string>\n\t\t<string>7777</string>\n",
        "<key>PATH</key>\n\t\t<string>/opt/homebrew/bin:/usr/bin:/bin</string>",
        "<key>WorkingDirectory</key>\n\t<string>/Users/op/.mergegate</string>",
        "<key>StandardOutPath</key>\n\t<string>/Users/op/.mergegate/hub.outcome</string>",
        "<key>StandardErrorPath</key>\n\t<string>/Users/op/.mergegate/hub.log</string>",
        "<key>RunAtLoad</key>\n\t<true/>",
        "<key>SuccessfulExit</key>\n\t\t<false/>",
    ] {
        assert!(plist.contains(line), "missing {line:?} in:\n{plist}");
    }
}

#[test]
fn only_an_agent_for_the_background_session_limits_itself_to_it() {
    let key = "<key>LimitLoadToSessionType</key>\n\t<string>Background</string>";
    let background = render(&spec("/opt/homebrew/bin/mg"), Session::Background);
    assert!(background.contains(key), "{background}");
    let gui = render(&spec("/opt/homebrew/bin/mg"), Session::Gui);
    assert!(!gui.contains("LimitLoadToSessionType"), "{gui}");
}

#[test]
fn the_binary_reads_back_through_xml_escapes() {
    let odd = "/Users/op/R&D <tools>/mg";
    let plist = render(&spec(odd), Session::Gui);
    assert!(plist.contains("R&amp;D &lt;tools&gt;"), "{plist}");
    assert_eq!(exe(&plist), Some(PathBuf::from(odd)));
}

#[test]
fn a_plist_without_program_arguments_has_no_binary() {
    assert_eq!(exe("<plist><dict></dict></plist>"), None);
}
