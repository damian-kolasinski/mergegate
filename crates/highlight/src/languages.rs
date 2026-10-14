//! The installed packs, one directory each under the hub's, as
//! `<name>/<version>/`, and the engine that runs their grammars.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError, RwLock};

use mergegate_core::highlight::{Highlighter, Highlights, Installed, LangError, Pack};
use sha2::{Digest, Sha256};
use tree_sitter::WasmStore;
use tree_sitter::wasmtime::Engine as Wasm;

use crate::load::{self, Loaded};
use crate::{archive, manifest, spans};

pub struct Languages {
    dir: PathBuf,
    /// By name. Read by requests, so never held while a grammar runs.
    installed: RwLock<Vec<Entry>>,
    /// Taken before `installed` by whatever takes both.
    engine: Mutex<Engine>,
}

struct Entry {
    pack: Pack,
    dir: PathBuf,
    /// The archive's SHA-256, so a pack repacked under the same version
    /// still changes the spans cache's version.
    digest: [u8; 32],
}

/// The archive digest kept in a pack's directory. Archives can't hold
/// dotfiles, so it never collides with the pack's own files.
const DIGEST: &str = ".digest";

/// Bump when mg cuts spans differently (`tags.rs`, `spans.rs`), so cached
/// spans are computed again.
const SPANS: u32 = 1;

struct Engine {
    /// Compiles grammars; any parser on the same wasm engine runs them.
    loader: WasmStore,
    highlighter: tree_sitter_highlight::Highlighter,
    /// Every installed grammar, compiled on the first highlight rather than
    /// when the hub starts: all of them take about half a second.
    loaded: Option<Vec<Loaded>>,
}

/// A pack unpacked and compiled, not yet moved into place.
struct Staged {
    archive: PathBuf,
    incoming: Incoming,
    pack: Pack,
    loaded: Vec<Loaded>,
    digest: [u8; 32],
}

/// A directory an install unpacks into, removed unless the install moves it
/// into place. One left by a hub that stopped mid-install goes at the next
/// start.
struct Incoming(PathBuf);

const INCOMING: &str = ".incoming-";

impl Incoming {
    fn create(dir: &Path) -> io::Result<Incoming> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!("{INCOMING}{}-{n}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Incoming(path))
    }
}

impl Drop for Incoming {
    fn drop(&mut self) {
        // Gone already once moved into place.
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl Languages {
    /// The packs installed in `dir`, created if missing, with a warning for
    /// each one that can't be read.
    pub fn open(dir: &Path) -> io::Result<(Languages, Vec<String>)> {
        fs::create_dir_all(dir)?;
        let mut installed = Vec::new();
        let mut warnings = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with(INCOMING) {
                let _ = fs::remove_dir_all(&path);
            } else if entry.file_type()?.is_dir() {
                match open_pack(&path) {
                    Ok(pack) => installed.push(pack),
                    Err(e) => {
                        warnings.push(format!("skipped language pack {}: {e}", path.display()))
                    }
                }
            }
        }
        installed.sort_by(|a, b| a.pack.name.cmp(&b.pack.name));
        let wasm = Wasm::default();
        let store = || WasmStore::new(&wasm).map_err(|e| io::Error::other(e.to_string()));
        let mut highlighter = tree_sitter_highlight::Highlighter::new();
        highlighter
            .parser()
            .set_wasm_store(store()?)
            .map_err(io::Error::other)?;
        let engine = Engine {
            loader: store()?,
            highlighter,
            loaded: None,
        };
        let languages = Languages {
            dir: dir.to_owned(),
            installed: RwLock::new(installed),
            engine: Mutex::new(engine),
        };
        Ok((languages, warnings))
    }

    fn engine(&self) -> MutexGuard<'_, Engine> {
        self.engine.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn entries(&self) -> std::sync::RwLockReadGuard<'_, Vec<Entry>> {
        self.installed
            .read()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn stage(&self, loader: &mut WasmStore, archive: &Path) -> Result<Staged, LangError> {
        let incoming = Incoming::create(&self.dir).map_err(failed)?;
        let invalid = |reason| LangError::Invalid {
            path: archive.to_owned(),
            reason,
        };
        let digest: [u8; 32] = Sha256::digest(fs::read(archive).map_err(failed)?).into();
        archive::unpack(archive, &incoming.0).map_err(invalid)?;
        fs::write(incoming.0.join(DIGEST), digest).map_err(failed)?;
        let pack =
            manifest::parse(&archive::manifest(&incoming.0).map_err(invalid)?).map_err(invalid)?;
        let loaded = pack
            .grammars
            .iter()
            .map(|g| load::load(loader, &pack.name, &incoming.0, g))
            .collect::<Result<_, _>>()
            .map_err(invalid)?;
        Ok(Staged {
            archive: archive.to_owned(),
            incoming,
            pack,
            loaded,
            digest,
        })
    }

    /// Fails unless every grammar name stays with one pack.
    fn check(&self, staged: &[Staged]) -> Result<(), LangError> {
        let installed = self.entries();
        for (i, s) in staged.iter().enumerate() {
            if staged[..i]
                .iter()
                .any(|other| other.pack.name == s.pack.name)
            {
                return Err(LangError::Invalid {
                    path: s.archive.clone(),
                    reason: format!("another archive is the {} pack too", s.pack.name),
                });
            }
            let others = installed
                .iter()
                .map(|i| &i.pack)
                .filter(|p| p.name != s.pack.name)
                .chain(staged[..i].iter().map(|s| &s.pack));
            for other in others {
                if let Some(g) = s
                    .pack
                    .grammars
                    .iter()
                    .find(|g| other.grammars.iter().any(|o| o.name == g.name))
                {
                    return Err(LangError::Taken {
                        grammar: g.name.clone(),
                        pack: other.name.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Moves `staged` into place, replacing every installed version.
    fn commit(&self, staged: &Staged) -> io::Result<PathBuf> {
        let pack = self.dir.join(&staged.pack.name);
        fs::create_dir_all(&pack)?;
        for entry in fs::read_dir(&pack)? {
            fs::remove_dir_all(entry?.path())?;
        }
        let target = pack.join(&staged.pack.version);
        fs::rename(&staged.incoming.0, &target)?;
        Ok(target)
    }
}

impl Highlighter for Languages {
    /// Versioned by every pack's archive digest and the span format.
    fn installed(&self) -> Installed {
        let entries = self.entries();
        let mut hash = Sha256::new();
        hash.update(SPANS.to_le_bytes());
        for e in entries.iter() {
            hash.update(e.digest);
        }
        let version = hash.finalize()[..16]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Installed {
            packs: entries.iter().map(|e| e.pack.clone()).collect(),
            version,
        }
    }

    fn install(&self, from: &Path) -> Result<Vec<Pack>, LangError> {
        let archives = archives(from)?;
        let mut engine = self.engine();
        let staged = archives
            .iter()
            .map(|a| self.stage(&mut engine.loader, a))
            .collect::<Result<Vec<_>, _>>()?;
        self.check(&staged)?;
        let mut installed = self
            .installed
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let mut packs = Vec::new();
        for s in staged {
            let dir = self.commit(&s).map_err(failed)?;
            installed.retain(|i| i.pack.name != s.pack.name);
            installed.push(Entry {
                pack: s.pack.clone(),
                dir,
                digest: s.digest,
            });
            if let Some(loaded) = &mut engine.loaded {
                loaded.retain(|l| l.pack != s.pack.name);
                loaded.extend(s.loaded);
            }
            packs.push(s.pack);
        }
        installed.sort_by(|a, b| a.pack.name.cmp(&b.pack.name));
        Ok(packs)
    }

    fn remove(&self, name: &str) -> Result<Pack, LangError> {
        let mut engine = self.engine();
        let mut installed = self
            .installed
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let index = installed
            .iter()
            .position(|i| i.pack.name == name)
            .ok_or_else(|| LangError::NotInstalled(name.into()))?;
        fs::remove_dir_all(self.dir.join(name)).map_err(failed)?;
        if let Some(loaded) = &mut engine.loaded {
            loaded.retain(|l| l.pack != name);
        }
        Ok(installed.remove(index).pack)
    }

    fn highlight(&self, grammar: &str, text: &str) -> Option<Highlights> {
        let mut engine = self.engine();
        let Engine {
            loader,
            highlighter,
            loaded,
        } = &mut *engine;
        let loaded: &[Loaded] = loaded.get_or_insert_with(|| self.load_all(loader));
        let config = &loaded.iter().find(|l| l.grammar.name == grammar)?.config;
        spans::highlight(highlighter, config, text, |name| {
            load::injected(loaded, name)
        })
        .ok()
    }
}

impl Languages {
    /// A grammar that loaded when it was installed but no longer does, its
    /// files changed since, is left out: its files show as plain text.
    fn load_all(&self, loader: &mut WasmStore) -> Vec<Loaded> {
        let installed = self.entries();
        let grammars = installed
            .iter()
            .flat_map(|i| i.pack.grammars.iter().map(move |g| (i, g)));
        grammars
            .filter_map(|(i, g)| load::load(loader, &i.pack.name, &i.dir, g).ok())
            .collect()
    }
}

/// `<dir>/<version>/`, holding the only version install leaves.
fn open_pack(dir: &Path) -> Result<Entry, String> {
    let versions: Vec<PathBuf> = fs::read_dir(dir)
        .and_then(|entries| entries.map(|e| e.map(|e| e.path())).collect())
        .map_err(|e| e.to_string())?;
    let [version] = versions.as_slice() else {
        return Err(format!("{} versions installed, not one", versions.len()));
    };
    let pack = manifest::parse(&archive::manifest(version)?)?;
    if dir.file_name() != Some(pack.name.as_ref())
        || version.file_name() != Some(pack.version.as_ref())
    {
        return Err(format!("its pack.json is {} {}", pack.name, pack.version));
    }
    let digest = fs::read(version.join(DIGEST))
        .ok()
        .and_then(|d| d.try_into().ok())
        .ok_or("no .digest; add the pack again")?;
    Ok(Entry {
        pack,
        dir: version.clone(),
        digest,
    })
}

/// `from` itself, or the `.tar.gz` files in it.
fn archives(from: &Path) -> Result<Vec<PathBuf>, LangError> {
    let invalid = |e: io::Error| LangError::Invalid {
        path: from.to_owned(),
        reason: e.to_string(),
    };
    if !fs::metadata(from).map_err(invalid)?.is_dir() {
        return Ok(vec![from.to_owned()]);
    }
    let mut found = Vec::new();
    for entry in fs::read_dir(from).map_err(invalid)? {
        let path = entry.map_err(invalid)?.path();
        if path.is_file() && path.to_string_lossy().ends_with(".tar.gz") {
            found.push(path);
        }
    }
    if found.is_empty() {
        return Err(LangError::Empty(from.to_owned()));
    }
    found.sort();
    Ok(found)
}

fn failed(e: io::Error) -> LangError {
    LangError::Failed(Box::new(e))
}
