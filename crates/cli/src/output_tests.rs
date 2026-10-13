use super::*;
use api::decision::DecisionOption;

fn decision() -> Outcome {
    Outcome::NeedsDecision(NeedsDecision {
        question: "Which backend should the project use?".into(),
        flag: "--backend".into(),
        options: vec![
            DecisionOption {
                value: "github".into(),
                description: Some("push branches and open PRs".into()),
            },
            DecisionOption {
                value: "local".into(),
                description: None,
            },
        ],
        default: Some("github".into()),
    })
}

#[test]
fn needs_decision_exits_2_with_the_documented_shape() {
    let (stdout, stderr, code) = render(true, Ok(decision()));
    assert_eq!(code, exit::NEEDS_DECISION);
    assert!(stderr.is_empty());
    let value: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"needs_decision": {
            "question": "Which backend should the project use?",
            "flag": "--backend",
            "options": [
                {"value": "github", "description": "push branches and open PRs"},
                {"value": "local"}
            ],
            "default": "github"
        }})
    );
}

#[test]
fn needs_decision_as_text_names_the_flag() {
    let (stdout, _, code) = render(false, Ok(decision()));
    assert_eq!(code, exit::NEEDS_DECISION);
    assert_eq!(
        stdout,
        "needs decision: Which backend should the project use?\n  \
             --backend github (default) · push branches and open PRs\n  \
             --backend local\n\
             re-run with --backend set to the chosen option"
    );
}

#[test]
fn errors_go_to_stdout_as_json_and_to_stderr_as_text() {
    let err = || CliError::new("port_in_use", "port 7800 is already in use").hint("--port N");
    let (stdout, stderr, code) = render(true, Err(err()));
    assert_eq!(code, exit::ERROR);
    assert!(stderr.is_empty());
    assert_eq!(
        serde_json::from_str::<Value>(&stdout).unwrap(),
        serde_json::json!({"error": {
            "code": "port_in_use",
            "message": "port 7800 is already in use",
            "hint": "--port N"
        }})
    );
    let (stdout, stderr, _) = render(false, Err(err()));
    assert!(stdout.is_empty());
    assert_eq!(stderr, "error: port 7800 is already in use\nhint: --port N");
}

#[test]
fn a_yes_or_no_decision_names_the_flag_alone() {
    let decision = Outcome::NeedsDecision(NeedsDecision {
        question: "1 open item keeps its target. Change it?".into(),
        flag: "--confirm".into(),
        options: Vec::new(),
        default: None,
    });
    let (stdout, _, code) = render(false, Ok(decision));
    assert_eq!(code, exit::NEEDS_DECISION);
    assert_eq!(
        stdout,
        "needs decision: 1 open item keeps its target. Change it?\n\
         re-run with --confirm to go ahead"
    );
}
