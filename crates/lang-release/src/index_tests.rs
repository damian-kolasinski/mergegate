use super::*;

const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn index(file: &str, sha256: &str) -> String {
    format!(
        r#"{{"packs":[{{"name":"json","version":"0.24.8","file":"{file}","sha256":"{sha256}"}}]}}"#
    )
}

#[test]
fn reads_the_packs_build_mjs_lists() {
    let index = Index::parse(&index("json-0.24.8.tar.gz", SHA)).unwrap();
    assert_eq!(index.packs.len(), 1);
    assert_eq!(index.packs[0].file, "json-0.24.8.tar.gz");
}

#[test]
fn archive_names_stay_inside_the_download_directory() {
    for file in ["../json.tar.gz", "a/json.tar.gz", ".tar.gz", "json.zip"] {
        let err = Index::parse(&index(file, SHA)).err().unwrap();
        assert!(err.contains("is not an archive name"), "{file}: {err}");
    }
}

#[test]
fn every_pack_has_a_sha256() {
    for sha in ["", "ABCDEF", &SHA.to_uppercase()] {
        assert!(Index::parse(&index("json-0.24.8.tar.gz", sha)).is_err());
    }
}
