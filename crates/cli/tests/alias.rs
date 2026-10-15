//! `mergegate`: the same CLI as `mg`, for a PATH where macOS's `/usr/bin/mg`
//! editor comes first.

use testkit::Env;

#[test]
fn mergegate_runs_the_mg_beside_it_with_its_exit_code() {
    let mg = Env::new(env!("CARGO_BIN_EXE_mg"));
    let mergegate = Env::new(env!("CARGO_BIN_EXE_mergegate"));
    assert_eq!(
        mergegate.mg(&["--version"]).assert_code(0).stdout,
        mg.mg(&["--version"]).assert_code(0).stdout
    );
    // No hub runs in its home, so doctor fails as mg's does.
    let doctor = mergegate.mg(&["--json", "doctor"]).assert_code(1).json();
    assert_eq!(doctor["checks"][0]["name"], "hub");
}
