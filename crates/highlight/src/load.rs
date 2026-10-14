//! Compiling an installed grammar: its wasm and queries, read from the
//! pack's directory.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use mergegate_core::highlight::Grammar;
use tree_sitter::WasmStore;
use tree_sitter_highlight::HighlightConfiguration;

use crate::tags;

pub(crate) struct Loaded {
    pub pack: String,
    pub grammar: Grammar,
    pub config: HighlightConfiguration,
}

/// `grammar` of `pack`, installed or staged in `dir`, compiled into `store`'s engine. A
/// language compiled in one store runs in any parser on the same engine.
pub(crate) fn load(
    store: &mut WasmStore,
    pack: &str,
    dir: &Path,
    grammar: &Grammar,
) -> Result<Loaded, String> {
    let name = &grammar.name;
    let wasm =
        fs::read(dir.join(format!("{name}.wasm"))).map_err(|e| format!("{name}.wasm: {e}"))?;
    let language = store
        .load_language(name, &wasm)
        .map_err(|e| format!("{name}.wasm: {e}"))?;
    let query = |kind: &str, required: bool| {
        let file = format!("{name}.{kind}.scm");
        match fs::read_to_string(dir.join(&file)) {
            Err(e) if e.kind() == ErrorKind::NotFound && !required => Ok(String::new()),
            read => read.map_err(|e| format!("{file}: {e}")),
        }
    };
    let highlights = query("highlights", true)?;
    let injections = query("injections", false)?;
    let locals = query("locals", false)?;
    let mut config = HighlightConfiguration::new(language, name, &highlights, &injections, &locals)
        .map_err(|e| format!("{name} queries: {e}"))?;
    config.configure(&tags::names());
    Ok(Loaded {
        pack: pack.into(),
        grammar: grammar.clone(),
        config,
    })
}

/// The grammar an injection names: by grammar name, alias, then extension,
/// as a Markdown fence may say `rust`, `rs` or `sh`.
pub(crate) fn injected<'a>(loaded: &'a [Loaded], name: &str) -> Option<&'a HighlightConfiguration> {
    let name = name.trim().to_ascii_lowercase();
    let find = |matches: &dyn Fn(&Grammar) -> bool| {
        loaded
            .iter()
            .find(|l| matches(&l.grammar))
            .map(|l| &l.config)
    };
    find(&|g| g.name == name)
        .or_else(|| find(&|g| g.aliases.iter().any(|a| a.eq_ignore_ascii_case(&name))))
        .or_else(|| find(&|g| g.extensions.iter().any(|e| e.eq_ignore_ascii_case(&name))))
}
