use mergegate_core::git::{Git, Upstream};
use mergegate_core::project::ProjectName;
use mergegate_core::triage::TriageGit;
use mergegate_git::HubGit;
use testkit::TestRepo;

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

#[test]
fn reads_linguist_generated_from_the_given_commit() {
    let dir = tempfile::tempdir().unwrap();
    let git = HubGit::new(dir.path().join("projects"), dir.path().join("git-home"));
    let repo = TestRepo::new("app");
    repo.commit_files(
        "Mark generated code",
        &[
            (
                ".gitattributes",
                "gen/* linguist-generated\nkept.ts -linguist-generated\n",
            ),
            ("gen/a.ts", "a\n"),
        ],
    );
    let marked = repo.rev("main");
    repo.commit_files("Unmark it", &[(".gitattributes", "\n")]);
    git.sync_staging(&app(), &Upstream::Repo(repo.root().into()), "main")
        .unwrap();

    let paths = ["gen/a.ts", "gen/new file.ts", "kept.ts", "src/b.ts"];
    assert_eq!(
        git.generated(&app(), &marked, &paths).unwrap(),
        [true, true, false, false]
    );
    let head = repo.rev("main");
    assert_eq!(git.generated(&app(), &head, &paths).unwrap(), [false; 4]);
    assert_eq!(
        git.generated(&app(), &head, &[]).unwrap(),
        Vec::<bool>::new()
    );
}
