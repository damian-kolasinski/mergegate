use super::*;
use crate::diff::{Hunk, Line};
use crate::item::{ChangedFile, FileMode};
use FileMode::{Submodule, Symlink};
use crate::triage::{Rule, RuleId};

/// A rule as `tier path [kind=K] [max=N] [diff=REGEX]`, `-` for no path.
fn rule(n: u32, spec: &str) -> Rule {
    let mut words = spec.split(' ');
    let tier = Tier::parse(words.next().unwrap()).unwrap();
    let path = words.next().unwrap();
    let mut rule = Rule {
        id: RuleId::new(n),
        tier,
        path: (path != "-").then(|| path.to_owned()),
        kind: None,
        max_lines: None,
        diff_matches: None,
        reason: "why".into(),
    };
    for word in words {
        match word.split_once('=').unwrap() {
            ("kind", kind) => rule.kind = Some(Kind::parse(kind).unwrap()),
            ("max", max) => rule.max_lines = Some(max.parse().unwrap()),
            ("diff", regex) => rule.diff_matches = Some(regex.to_owned()),
            (other, _) => panic!("unknown matcher {other}"),
        }
    }
    rule
}

/// A file as `path [from=OLD] [added|deleted|generated|binary|large]
/// [chmod|symlink|to-symlink|submodule] [+N] [-N]`, its diff the lines given
/// with it.
fn file(spec: &str) -> (FileStat, Option<Content>) {
    let mut words = spec.split(' ');
    let path = words.next().unwrap().to_owned();
    let mut stat = FileStat {
        file: ChangedFile {
            path,
            old_path: None,
            old_blob: Some("old".into()),
            new_blob: Some("new".into()),
            old_mode: Some(FileMode::Regular),
            new_mode: Some(FileMode::Regular),
        },
        additions: Some(1),
        deletions: Some(1),
        generated: false,
        triage: Triage::default(),
    };
    let mut content = Some(Content::Text(Vec::new()));
    for word in words {
        match word {
            "added" => (stat.file.old_blob, stat.file.old_mode) = (None, None),
            "deleted" => (stat.file.new_blob, stat.file.new_mode) = (None, None),
            "generated" => stat.generated = true,
            "binary" => {
                (stat.additions, stat.deletions) = (None, None);
                content = Some(Content::Binary);
            }
            "large" => content = Some(Content::TooLarge),
            "chmod" => stat.file.new_mode = Some(FileMode::Executable),
            "symlink" => (stat.file.old_mode, stat.file.new_mode) = (Some(Symlink), Some(Symlink)),
            "to-symlink" => stat.file.new_mode = Some(Symlink),
            "submodule" => {
                (stat.file.old_mode, stat.file.new_mode) = (Some(Submodule), Some(Submodule));
            }
            word if word.starts_with("from=") => stat.file.old_path = Some(word[5..].into()),
            word if word.starts_with('+') => stat.additions = Some(word[1..].parse().unwrap()),
            word if word.starts_with('-') => stat.deletions = Some(word[1..].parse().unwrap()),
            other => panic!("unknown file word {other}"),
        }
    }
    (stat, content)
}

fn lines(lines: &[&str]) -> Content {
    let lines = lines
        .iter()
        .map(|line| {
            let (kind, text) = match line.split_at(1) {
                ("+", text) => (LineKind::Added, text),
                ("-", text) => (LineKind::Removed, text),
                (_, text) => (LineKind::Context, text),
            };
            Line {
                kind,
                text: text.into(),
                newline: true,
            }
        })
        .collect();
    Content::Text(vec![Hunk {
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 1,
        lines,
    }])
}

/// The outcome as `tier [R<n>|guard]`.
fn outcome(triage: &Triage) -> String {
    let by = match &triage.by {
        None => String::new(),
        Some(TriagedBy::Guard(_)) => " guard".into(),
        Some(TriagedBy::Rule { id, .. }) => format!(" {id}"),
    };
    format!("{}{by}", triage.tier.as_str())
}

struct Case {
    name: &'static str,
    rules: &'static [&'static str],
    file: &'static str,
    diff: &'static [&'static str],
    expect: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "no rules: review",
        rules: &[],
        file: "src/a.rs",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "**/ matches the root",
        rules: &["trusted **/*.lock"],
        file: "Cargo.lock",
        diff: &[],
        expect: "trusted R1",
    },
    Case {
        name: "* stays in its directory",
        rules: &["trusted *.lock"],
        file: "ui/pnpm.lock",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "first match wins: review",
        rules: &["review src/**", "trusted **/*.rs"],
        file: "src/a.rs",
        diff: &[],
        expect: "review R1",
    },
    Case {
        name: "first match wins: trusted",
        rules: &["review src/**", "trusted **/*.rs"],
        file: "tests/a.rs",
        diff: &[],
        expect: "trusted R2",
    },
    Case {
        name: "guard: root .gitattributes",
        rules: &["trusted **"],
        file: ".gitattributes",
        diff: &[],
        expect: "review guard",
    },
    Case {
        name: "guard: nested .gitattributes",
        rules: &["trusted **"],
        file: "ui/.gitattributes",
        diff: &[],
        expect: "review guard",
    },
    Case {
        name: "guard: renamed away from .gitattributes",
        rules: &["trusted **"],
        file: "attributes from=.gitattributes",
        diff: &[],
        expect: "review guard",
    },
    Case {
        name: "guard: only the exact name",
        rules: &["trusted **"],
        file: "docs/.gitattributes.md",
        diff: &[],
        expect: "trusted R1",
    },
    Case {
        name: "trusted rename needs both paths",
        rules: &["trusted tests/**"],
        file: "tests/a.rs from=src/a.rs",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "trusted rename within the glob",
        rules: &["trusted tests/**"],
        file: "tests/b.rs from=tests/a.rs",
        diff: &[],
        expect: "trusted R1",
    },
    Case {
        name: "review rename needs either path",
        rules: &["review src/**", "trusted **"],
        file: "lib/a.rs from=src/a.rs",
        diff: &[],
        expect: "review R1",
    },
    Case {
        name: "a deleted file matches by its path",
        rules: &["trusted fixtures/**"],
        file: "fixtures/a.json deleted",
        diff: &[],
        expect: "trusted R1",
    },
    Case {
        name: "kind generated",
        rules: &["trusted - kind=generated"],
        file: "api/types.ts generated",
        diff: &[],
        expect: "trusted R1",
    },
    Case {
        name: "kind generated, not generated",
        rules: &["trusted - kind=generated"],
        file: "api/types.ts",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "kind added",
        rules: &["trusted snapshots/** kind=added"],
        file: "snapshots/a.snap added",
        diff: &[],
        expect: "trusted R1",
    },
    Case {
        name: "kind added, modified",
        rules: &["trusted snapshots/** kind=added"],
        file: "snapshots/a.snap",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "kind deleted",
        rules: &["trusted - kind=deleted"],
        file: "old.rs deleted",
        diff: &[],
        expect: "trusted R1",
    },
    Case {
        name: "kind renamed",
        rules: &["trusted - kind=renamed max=0"],
        file: "b.rs from=a.rs +0 -0",
        diff: &[],
        expect: "trusted R1",
    },
    Case {
        name: "kind modified excludes a rename",
        rules: &["trusted - kind=modified"],
        file: "b.rs from=a.rs",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "max_lines counts both sides",
        rules: &["trusted **/*.md max=5"],
        file: "README.md +3 -2",
        diff: &[],
        expect: "trusted R1",
    },
    Case {
        name: "max_lines over",
        rules: &["trusted **/*.md max=5"],
        file: "README.md +3 -3",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "max_lines can't count a binary: trusted doesn't match",
        rules: &["trusted **/*.png max=5"],
        file: "logo.png binary",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "max_lines can't count a binary: review matches",
        rules: &["review assets/** max=5", "trusted **"],
        file: "assets/logo.png binary",
        diff: &[],
        expect: "review R1",
    },
    Case {
        name: "diff_matches an added line",
        rules: &["trusted **/package.json diff=^\\s*\"version\""],
        file: "package.json",
        diff: &[
            " {",
            "-  \"version\": \"1.0.0\",",
            "+  \"version\": \"1.0.1\",",
        ],
        expect: "trusted R1",
    },
    Case {
        name: "diff_matches ignores context",
        rules: &[
            "review **/*.swift diff=NavigationStack",
            "trusted **/*View.swift",
        ],
        file: "LoginView.swift",
        diff: &[" NavigationStack {", "+  Text(\"hi\")"],
        expect: "trusted R2",
    },
    Case {
        name: "diff_matches a removed line",
        rules: &[
            "review **/*.swift diff=NavigationStack",
            "trusted **/*View.swift",
        ],
        file: "LoginView.swift",
        diff: &["-NavigationStack {", "+VStack {"],
        expect: "review R1",
    },
    Case {
        name: "diff_matches hides only when every changed line matches",
        rules: &["trusted **/*.py diff=^import"],
        file: "app.py",
        diff: &["+import os", "+os.remove('/')"],
        expect: "review",
    },
    Case {
        name: "diff_matches can't hide a file with no changed lines",
        rules: &["trusted ** diff=.*"],
        file: "empty.txt",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "diff_matches can't read a large diff: trusted doesn't match",
        rules: &["trusted **/*.json diff=version"],
        file: "data.json large",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "diff_matches can't read a binary: review matches",
        rules: &["review **/*.bin diff=x", "trusted **"],
        file: "blob.bin binary",
        diff: &[],
        expect: "review R1",
    },
    Case {
        name: "a trusted rule can't hide a mode change",
        rules: &["trusted scripts/**"],
        file: "scripts/run.sh chmod +0 -0",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "a trusted rule can't hide a type change",
        rules: &["trusted docs/** kind=modified"],
        file: "docs/a.md to-symlink",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "a trusted rule can't hide a symlink's new target",
        rules: &["trusted docs/**"],
        file: "docs/link symlink",
        diff: &["-guide.md", "+/etc/passwd"],
        expect: "review",
    },
    Case {
        name: "a trusted rule can't hide a new symlink",
        rules: &["trusted docs/** kind=added"],
        file: "docs/link added to-symlink",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "a trusted rule can't hide a submodule",
        rules: &["trusted vendor/**"],
        file: "vendor/dep submodule binary",
        diff: &[],
        expect: "review",
    },
    Case {
        name: "a mode change falls through to a later rule",
        rules: &["trusted scripts/**", "review scripts/**"],
        file: "scripts/run.sh chmod",
        diff: &[],
        expect: "review R2",
    },
    Case {
        name: "a review rule still matches a mode change",
        rules: &["review scripts/**", "trusted **"],
        file: "scripts/run.sh chmod",
        diff: &[],
        expect: "review R1",
    },
];

#[test]
fn classifies() {
    let mut failed = Vec::new();
    for case in CASES {
        let rules = (1..)
            .zip(case.rules)
            .map(|(n, spec)| rule(n, spec))
            .collect();
        let rules = Rules::new(rules).unwrap();
        let (stat, content) = file(case.file);
        let content = match case.diff {
            [] => content,
            diff => Some(lines(diff)),
        };
        let triage = classify(&rules, &stat, || Ok::<_, ()>(content)).unwrap();
        let got = outcome(&triage);
        if got != case.expect {
            failed.push(format!(
                "{}: got {got:?}, want {:?}",
                case.name, case.expect
            ));
        }
    }
    assert!(failed.is_empty(), "\n{}", failed.join("\n"));
}

#[test]
fn reads_the_diff_only_when_a_rule_gets_to_it() {
    let rules = Rules::new(vec![rule(1, "trusted docs/** diff=x")]).unwrap();
    let (stat, _) = file("src/a.rs");
    let triage = classify(&rules, &stat, || -> Result<_, ()> {
        panic!("read the diff of a file the path ruled out")
    });
    assert_eq!(triage, Ok(Triage::default()));
}

#[test]
fn a_rule_explains_itself() {
    let rules = Rules::new(vec![rule(3, "trusted **/*.lock")]).unwrap();
    let (stat, _) = file("Cargo.lock");
    let triage = classify(&rules, &stat, || Ok::<_, ()>(None)).unwrap();
    assert_eq!(
        triage.by,
        Some(TriagedBy::Rule {
            id: RuleId::new(3),
            why: "`**/*.lock` — why".into()
        })
    );
}
