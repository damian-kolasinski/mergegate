use mergegate_core::git::{Git, Upstream};
use mergegate_core::module::{Entry, ModuleGit};
use mergegate_core::project::ProjectName;
use mergegate_git::HubGit;
use testkit::TestRepo;

fn app() -> ProjectName {
    ProjectName::parse("app").unwrap()
}

fn entry(name: &str, dir: bool) -> Entry {
    Entry {
        name: name.into(),
        dir,
    }
}

#[test]
fn lists_each_folder_of_the_given_commit() {
    let dir = tempfile::tempdir().unwrap();
    let git = HubGit::new(dir.path().join("projects"), dir.path().join("git-home"));
    let repo = TestRepo::new("app");
    repo.commit_files(
        "Add a module",
        &[
            ("Chat/Chat/View.swift", "a\n"),
            ("Chat/ChatTests/ViewTests.swift", "a\n"),
            ("Chat/Project.swift", "a\n"),
        ],
    );
    let head = repo.rev("main");
    git.sync_staging(&app(), &Upstream::Repo(repo.root().into()), "main")
        .unwrap();

    let listed = git
        .listings(
            &app(),
            &head,
            &["Chat", "Chat/Chat", "Gone", "Chat/Project.swift"],
        )
        .unwrap();
    assert_eq!(
        listed,
        [
            vec![
                entry("Chat", true),
                entry("ChatTests", true),
                entry("Project.swift", false),
            ],
            vec![entry("View.swift", false)],
            Vec::new(),
            Vec::new(),
        ]
    );
}
