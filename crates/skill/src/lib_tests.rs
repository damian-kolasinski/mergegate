use std::path::Path;

use super::*;

#[test]
fn lists_every_file_in_the_folder() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(NAME);
    let mut on_disk = Vec::new();
    let mut dirs = vec![root.clone()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                let relative = path.strip_prefix(&root).unwrap();
                on_disk.push(relative.to_str().unwrap().to_owned());
            }
        }
    }
    on_disk.sort();
    let mut listed: Vec<_> = FILES.iter().map(|(path, _)| (*path).to_owned()).collect();
    listed.sort();
    assert_eq!(listed, on_disk);
}

#[test]
fn names_itself_in_its_frontmatter() {
    assert!(FILES[0].1.starts_with(&format!("---\nname: {NAME}\n")));
}

#[test]
fn the_body_drops_the_frontmatter() {
    assert!(body().starts_with("# mergegate\n"));
}

#[test]
fn every_link_resolves() {
    for (path, text) in FILES {
        let dir = Path::new(path).parent().unwrap();
        for link in text.split("](").skip(1) {
            let target = link.split(')').next().unwrap();
            let resolved = normalize(&dir.join(target));
            assert!(
                FILES.iter().any(|(p, _)| Path::new(p) == resolved),
                "{path} links to {target}"
            );
        }
    }
}

fn normalize(path: &Path) -> std::path::PathBuf {
    let mut out = std::path::PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}
