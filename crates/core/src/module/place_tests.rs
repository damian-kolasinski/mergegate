use super::*;

/// Listings of every folder above `files`, as a tree holding just them.
fn tree(files: &[&str]) -> BTreeMap<String, Vec<Entry>> {
    let mut listings: BTreeMap<String, BTreeSet<Entry>> = BTreeMap::new();
    for file in files {
        let segments: Vec<&str> = file.split('/').collect();
        for depth in 1..segments.len() {
            listings
                .entry(segments[..depth].join("/"))
                .or_default()
                .insert(Entry {
                    name: segments[depth].into(),
                    dir: depth + 1 < segments.len(),
                });
        }
    }
    listings
        .into_iter()
        .map(|(dir, entries)| (dir, entries.into_iter().collect()))
        .collect()
}

fn placed(module: &str, target: Option<&str>) -> Option<Placement> {
    Some(Placement {
        module: module.into(),
        target: target.map(Into::into),
    })
}

const TUIST: [&str; 9] = [
    "Project.swift",
    "Products/Messenger/ChatGroups/ChatGroups/Views/GroupView.swift",
    "Products/Messenger/ChatGroups/ChatGroupsInterface/GroupsAPI.swift",
    "Products/Messenger/ChatGroups/ChatGroupsTests/Views/GroupView.tests.swift",
    "Products/Messenger/ChatGroups/ChatGroups/Resources/Assets.xcassets/Back/Contents.json",
    "Products/Messenger/AppShell/AppShell/Shell.swift",
    "Platform/Networking/Networking/Client.swift",
    "Platform/Networking/NetworkingTests/ClientTests.swift",
    "Scripts/lint.sh",
];

#[test]
fn a_folder_holding_a_target_named_for_it_is_a_module() {
    let listings = tree(&TUIST);
    let group = "Products/Messenger/ChatGroups";
    assert_eq!(
        place(TUIST[1], &listings),
        placed(group, Some("ChatGroups"))
    );
    assert_eq!(
        place(TUIST[2], &listings),
        placed(group, Some("ChatGroupsInterface"))
    );
    assert_eq!(
        place(TUIST[3], &listings),
        placed(group, Some("ChatGroupsTests"))
    );
    // A bundle that holds a folder named like it isn't one.
    assert_eq!(
        place(TUIST[4], &listings),
        placed(group, Some("ChatGroups"))
    );
    assert_eq!(
        place(TUIST[6], &listings),
        placed("Platform/Networking", Some("Networking"))
    );
}

#[test]
fn the_root_is_never_a_module() {
    let listings = tree(&TUIST);
    assert_eq!(place("Project.swift", &listings), None);
    assert_eq!(place("Scripts/lint.sh", &listings), None);
}

const CARGO: [&str; 7] = [
    "Cargo.toml",
    "crates/hub/Cargo.toml",
    "crates/hub/src/diff.rs",
    "crates/hub/tests/items.rs",
    "ui/package.json",
    "ui/item/files.tsx",
    ".claude/skills/ship/SKILL.md",
];

#[test]
fn a_folder_with_a_manifest_is_a_module() {
    let listings = tree(&CARGO);
    assert_eq!(place(CARGO[1], &listings), placed("crates/hub", None));
    assert_eq!(
        place(CARGO[2], &listings),
        placed("crates/hub", Some("src"))
    );
    assert_eq!(
        place(CARGO[3], &listings),
        placed("crates/hub", Some("tests"))
    );
    assert_eq!(place(CARGO[5], &listings), placed("ui", None));
    assert_eq!(place(CARGO[6], &listings), None);
}

#[test]
fn the_nearest_module_wins() {
    let files = [
        "app/package.json",
        "app/packages/web/package.json",
        "app/packages/web/src/main.ts",
    ];
    let listings = tree(&files);
    assert_eq!(
        place(files[2], &listings),
        placed("app/packages/web", Some("src"))
    );
}

#[test]
fn a_package_can_have_a_dot_in_its_name() {
    let files = [
        "packages/socket.io/package.json",
        "packages/socket.io/lib/index.js",
        "src/Company.Api/Company.Api.csproj",
        "src/Company.Api/Program.cs",
        ".github/package.json",
        ".github/workflows/ci.yml",
    ];
    let listings = tree(&files);
    assert_eq!(
        place(files[1], &listings),
        placed("packages/socket.io", Some("lib"))
    );
    assert_eq!(place(files[3], &listings), placed("src/Company.Api", None));
    // Hidden folders aren't, manifest or not.
    assert_eq!(place(files[5], &listings), None);
}

#[test]
fn a_target_says_its_layer() {
    let at = |target: &str| Placement {
        module: "Products/Messenger/ChatGroups".into(),
        target: Some(target.into()),
    };
    assert_eq!(at("ChatGroups").layer(), None);
    assert_eq!(at("ChatGroupsInterface").layer(), Some("Interface"));
    assert_eq!(at("ChatGroupsTests").layer(), Some("Tests"));
    assert_eq!(at("src").layer(), None);
    assert_eq!(at("tests").layer(), Some("Tests"));
    assert_eq!(placed("crates/hub", None).unwrap().layer(), None);
    let sources = placed("Packages/Source", Some("Sources")).unwrap();
    assert_eq!(sources.layer(), None);
}

#[test]
fn a_path_within_its_module_leaves_the_target_out() {
    let listings = tree(&TUIST);
    let inner = |path: &str| place(path, &listings).unwrap().inner(path).to_owned();
    assert_eq!(inner(TUIST[1]), "Views/GroupView.swift");
    assert_eq!(inner(TUIST[3]), "Views/GroupView.tests.swift");
    let listings = tree(&CARGO);
    let inner = |path: &str| place(path, &listings).unwrap().inner(path).to_owned();
    assert_eq!(inner(CARGO[1]), "Cargo.toml");
    assert_eq!(inner(CARGO[2]), "diff.rs");
}

#[test]
fn ancestors_leave_the_root_out() {
    let dirs = ancestors(["a/b/c.rs", "a/d.rs", "e.rs"]);
    assert_eq!(dirs, BTreeSet::from(["a".into(), "a/b".into()]));
}

#[test]
fn a_file_adopts_the_nearest_module_above_it() {
    let modules = ["crates/hub", "Products/Messenger/ChatGroups", "crates"];
    assert_eq!(
        adopt("crates/hub/src/gone.rs", modules),
        placed("crates/hub", Some("src"))
    );
    assert_eq!(
        adopt(
            "Products/Messenger/ChatGroups/ChatGroupsTests/A.swift",
            modules
        ),
        placed("Products/Messenger/ChatGroups", Some("ChatGroupsTests"))
    );
    assert_eq!(adopt("crates-x/a.rs", modules), None);
    assert_eq!(adopt("README.md", modules), None);
}
