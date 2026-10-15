//! The language packs released with this mg: the index of them baked in at
//! build time, and their archives, downloaded from the release and checked
//! against it.

mod index;

use std::io::Read;
use std::path::Path;
use std::time::Duration;
use std::{env, fs};

use mergegate_core::highlight::{LangError, PackSource, Release};
use sha2::{Digest, Sha256};
use ureq::Agent;

use crate::index::Index;

/// The index `build.rs` baked in.
const BAKED: &str = include_str!(concat!(env!("OUT_DIR"), "/languages.json"));

/// Where this mg's release keeps its assets.
const RELEASE: &str = concat!(
    env!("CARGO_PKG_REPOSITORY"),
    "/releases/download/v",
    env!("CARGO_PKG_VERSION")
);

/// The largest archive downloaded. The biggest packs are a few MB.
const MAX_BYTES: u64 = 64 << 20;

pub struct Releases {
    releases: Vec<Release>,
    /// The archive of each of `releases`, at the same index.
    files: Vec<String>,
    base: String,
    agent: Agent,
}

impl Releases {
    /// The baked index and this mg's release. `MG_LANGUAGES_INDEX` names
    /// another index and `MG_LANGUAGES_URL` another place to download from,
    /// for tests and for trying packs before they're released.
    pub fn from_env() -> Result<Releases, String> {
        let index = match env::var_os("MG_LANGUAGES_INDEX") {
            Some(path) => fs::read_to_string(&path).map_err(|e| {
                format!(
                    "can't read MG_LANGUAGES_INDEX {}: {e}",
                    Path::new(&path).display()
                )
            })?,
            None => BAKED.to_owned(),
        };
        let base = env::var("MG_LANGUAGES_URL").unwrap_or_else(|_| RELEASE.to_owned());
        Releases::new(&index, base)
    }

    /// None, as in a build without its release's index.
    pub fn none() -> Releases {
        Releases::new(r#"{"packs":[]}"#, RELEASE.to_owned()).expect("an empty index")
    }

    /// The packs `index` lists, downloaded from under `base`.
    pub fn new(index: &str, base: String) -> Result<Releases, String> {
        let index = Index::parse(index)?;
        let agent = Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(120)))
            .user_agent("mergegate")
            .build()
            .into();
        let (releases, files) = index
            .packs
            .into_iter()
            .map(|p| {
                let release = Release {
                    name: p.name,
                    version: p.version,
                    sha256: p.sha256,
                };
                (release, p.file)
            })
            .unzip();
        Ok(Releases {
            releases,
            files,
            base: base.trim_end_matches('/').to_owned(),
            agent,
        })
    }

    fn fetch(&self, file: &str, sha256: &str) -> Result<Vec<u8>, String> {
        let url = format!("{}/{file}", self.base);
        let response = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| format!("{url}: {e}"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("{url} answered {status}"));
        }
        let mut bytes = Vec::new();
        response
            .into_body()
            .into_reader()
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{url}: {e}"))?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(format!("{url} is over {} MB", MAX_BYTES >> 20));
        }
        let digest: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if digest != sha256 {
            return Err(format!(
                "{url} has SHA-256 {digest}, not the {sha256} this mg was released with"
            ));
        }
        Ok(bytes)
    }
}

impl PackSource for Releases {
    fn releases(&self) -> &[Release] {
        &self.releases
    }

    fn download(&self, release: &Release, into: &Path) -> Result<(), LangError> {
        let failed = |reason: String| LangError::Download {
            name: release.name.clone(),
            reason,
        };
        let i = self
            .releases
            .iter()
            .position(|r| r == release)
            .ok_or_else(|| LangError::NotReleased(release.name.clone()))?;
        let file = &self.files[i];
        let bytes = self.fetch(file, &release.sha256).map_err(failed)?;
        fs::write(into.join(file), bytes).map_err(|e| failed(e.to_string()))
    }
}
