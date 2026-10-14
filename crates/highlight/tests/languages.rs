use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use mergegate_core::highlight::{Highlighter, Highlights, LangError, Span, Tag};
use mergegate_highlight::Languages;

const JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/json-0.24.8.tar.gz"
);

fn open(dir: &Path) -> Languages {
    let (languages, warnings) = Languages::open(dir).unwrap();
    assert_eq!(warnings, Vec::<String>::new());
    languages
}

fn span(start: u32, len: u32, tag: Tag) -> Span {
    Span { start, len, tag }
}

fn names(languages: &Languages) -> Vec<(String, String)> {
    let packs = languages.packs();
    packs.into_iter().map(|p| (p.name, p.version)).collect()
}

/// The json fixture with `edit` applied to its pack.json.
fn repack(dir: &Path, file: &str, edit: impl Fn(String) -> String) -> PathBuf {
    let mut from = tar::Archive::new(GzDecoder::new(File::open(JSON).unwrap()));
    let path = dir.join(file);
    let mut to = tar::Builder::new(GzEncoder::new(
        File::create(&path).unwrap(),
        Compression::fast(),
    ));
    for entry in from.entries().unwrap() {
        let mut entry = entry.unwrap();
        let name = entry.path().unwrap().to_string_lossy().into_owned();
        if entry.header().entry_type().is_dir() {
            continue;
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        if name.ends_with("pack.json") {
            data = edit(String::from_utf8(data).unwrap()).into_bytes();
        }
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        to.append_data(&mut header, &name, data.as_slice()).unwrap();
    }
    to.into_inner().unwrap().finish().unwrap();
    path
}

#[test]
fn highlights_with_an_installed_pack() {
    let dir = tempfile::tempdir().unwrap();
    let languages = open(dir.path());
    assert_eq!(languages.highlight("json", "{}"), None);

    let installed = languages.install(Path::new(JSON)).unwrap();
    assert_eq!(installed[0].name, "json");
    assert!(installed[0].grammars[0].extensions.contains(&"json".into()));
    assert_eq!(names(&languages), [("json".into(), "0.24.8".into())]);
    let text = "{\"a\": 1,\n \"é\": true}\n";
    let expected = Highlights {
        lines: vec![
            vec![span(1, 3, Tag::String), span(6, 1, Tag::Number)],
            vec![span(1, 3, Tag::String), span(6, 4, Tag::Constant)],
            vec![],
        ],
    };
    assert_eq!(languages.highlight("json", text), Some(expected.clone()));
    assert_eq!(languages.highlight("rust", text), None);

    let reopened = open(dir.path());
    assert_eq!(names(&reopened), names(&languages));
    assert_eq!(reopened.highlight("json", text), Some(expected));
}

#[test]
fn installs_every_pack_in_a_directory_replacing_other_versions() {
    let dir = tempfile::tempdir().unwrap();
    let packs = tempfile::tempdir().unwrap();
    let languages = open(dir.path());
    languages.install(Path::new(JSON)).unwrap();
    repack(packs.path(), "json-9.tar.gz", |m| m.replace("0.24.8", "9"));
    fs::write(packs.path().join("notes.txt"), "not a pack").unwrap();

    let installed = languages.install(packs.path()).unwrap();
    assert_eq!(installed.len(), 1);
    assert_eq!(names(&languages), [("json".into(), "9".into())]);
    assert!(!dir.path().join("json/0.24.8").exists());
    assert!(languages.highlight("json", "1").is_some());
    assert_eq!(names(&open(dir.path())), [("json".into(), "9".into())]);
}

#[test]
fn installs_nothing_unless_every_pack_loads() {
    let dir = tempfile::tempdir().unwrap();
    let packs = tempfile::tempdir().unwrap();
    let languages = open(dir.path());
    fs::copy(JSON, packs.path().join("a.tar.gz")).unwrap();
    let bad = repack(packs.path(), "b.tar.gz", |m| {
        m.replace("\"json\",", "\"jsonc\",")
    });

    let err = languages.install(packs.path()).unwrap_err();
    assert!(
        matches!(&err, LangError::Invalid { path, .. } if *path == bad),
        "{err}"
    );
    assert!(err.to_string().contains("jsonc.wasm"), "{err}");
    assert!(languages.packs().is_empty());
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        0,
        "staging is cleaned up"
    );
}

#[test]
fn refuses_a_grammar_another_pack_has() {
    let dir = tempfile::tempdir().unwrap();
    let packs = tempfile::tempdir().unwrap();
    let languages = open(dir.path());
    languages.install(Path::new(JSON)).unwrap();
    let other = repack(packs.path(), "other.tar.gz", |m| {
        m.replacen("\"name\": \"json\"", "\"name\": \"other\"", 1)
    });

    let err = languages.install(&other).unwrap_err();
    assert!(
        matches!(&err, LangError::Taken { grammar, pack } if grammar == "json" && pack == "json"),
        "{err}"
    );
    assert_eq!(names(&languages), [("json".into(), "0.24.8".into())]);
}

#[test]
fn says_what_it_cannot_install() {
    let dir = tempfile::tempdir().unwrap();
    let empty = tempfile::tempdir().unwrap();
    let languages = open(dir.path());
    let missing = empty.path().join("gone.tar.gz");
    assert!(matches!(
        languages.install(empty.path()),
        Err(LangError::Empty(_))
    ));
    assert!(matches!(
        languages.install(&missing),
        Err(LangError::Invalid { .. })
    ));
    assert!(matches!(
        languages.remove("json"),
        Err(LangError::NotInstalled(_))
    ));
}

#[test]
fn removes_a_pack() {
    let dir = tempfile::tempdir().unwrap();
    let languages = open(dir.path());
    languages.install(Path::new(JSON)).unwrap();
    assert!(languages.highlight("json", "1").is_some());

    assert_eq!(languages.remove("json").unwrap().name, "json");
    assert!(languages.packs().is_empty());
    assert_eq!(languages.highlight("json", "1"), None);
    assert!(!dir.path().join("json").exists());
}

#[test]
fn opening_clears_an_interrupted_install_and_skips_a_broken_pack() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".incoming-1-0")).unwrap();
    fs::create_dir_all(dir.path().join("broken/1")).unwrap();
    let (languages, warnings) = Languages::open(dir.path()).unwrap();
    assert!(languages.packs().is_empty());
    assert!(!dir.path().join(".incoming-1-0").exists());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("broken"), "{warnings:?}");
}

/// Every pack `langs/build.mjs` built, in the directory `MG_LANG_PACKS`
/// names: CI runs this after building them.
#[test]
#[ignore = "needs the built language packs"]
fn every_built_pack_loads_and_highlights() {
    let packs = PathBuf::from(std::env::var("MG_LANG_PACKS").expect("MG_LANG_PACKS"));
    let dir = tempfile::tempdir().unwrap();
    let languages = open(dir.path());
    let installed = languages.install(&packs).unwrap();
    let known = mergegate_highlight::known();
    assert_eq!(installed.len(), known.len());
    let samples = [
        ("bash", "echo \"hi\" # x"),
        ("css", "a { color: red; }"),
        ("elixir", "def x, do: :ok"),
        ("go", "func main() {}"),
        ("html", "<p class=\"x\">hi</p>"),
        ("javascript", "const a = 1;"),
        ("json", "{\"a\": 1}"),
        ("kotlin", "fun main() {}"),
        ("markdown", "# Title"),
        ("markdown_inline", "*a* `b`"),
        ("python", "def f(): return 1"),
        ("rust", "fn main() {}"),
        ("swift", "func f() {}"),
        ("toml", "a = 1"),
        ("tsx", "const a = <div />;"),
        ("typescript", "let a: number = 1;"),
        ("yaml", "a: 1"),
    ];
    for pack in &installed {
        for g in &pack.grammars {
            let (_, text) = samples.iter().find(|s| s.0 == g.name).expect(&g.name);
            let spans = languages.highlight(&g.name, text).expect(&g.name);
            assert!(!spans.lines[0].is_empty(), "{}: no spans", g.name);
        }
    }

    // Injections: a fenced block, inline markdown, and script in HTML.
    let md = languages
        .highlight("markdown", "# T\n\n```rust\nfn x() {}\n```\n")
        .unwrap();
    assert!(md.line(4).iter().any(|s| s.tag == Tag::Keyword), "{md:?}");
    let md = languages
        .highlight("markdown", "some *emphasis*\n")
        .unwrap();
    assert!(md.line(1).iter().any(|s| s.tag == Tag::Emphasis), "{md:?}");
    let html = languages
        .highlight("html", "<script>const a = 1;</script>")
        .unwrap();
    assert!(
        html.line(1).iter().any(|s| s.tag == Tag::Keyword),
        "{html:?}"
    );
}
