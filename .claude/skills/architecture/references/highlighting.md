# Highlighting and language packs

## Pipeline

The hub parses the full old and new blob with tree-sitter at submit time. It stores per-line token spans (`[start, len, class]`) keyed by `(blob, language, grammar version)`. The UI receives text plus spans and paints CSS classes, so themes cost nothing. The same parse feeds structural triage signals.

## Language packs

Languages are optional downloads, managed per hub. Only the hub highlights, so workers never need grammars and `mg` ships with none.

A pack contains:
- the grammar compiled to WASM: one file for every OS/arch, sandboxed, no native code loaded;
- the tree-sitter queries `highlights.scm` (plus `injections.scm` and `locals.scm` where present);
- a manifest entry: name, file extensions/filenames, version, tree-sitter ABI, sha256.

Grammars load through tree-sitter's WASM store (wasmtime), the mechanism Zed uses for extension grammars. WASM parsing is slower than native, but it runs once per blob at submit time and is cached. Wasmtime is a fixed binary-size cost, paid once instead of per grammar.

Distribution and storage:
- packs are GitHub release assets of the mergegate repo, so there's no hosted infrastructure;
- each `mg` version pins the index URL and the checksums it accepts;
- packs are stored in `~/.mergegate/languages/<name>/<version>/`.

## Commands

```
mg lang list [--available]              Installed languages (and what can be installed)
mg lang add <name>... | --detect [<project>]
mg lang add --from FILE                 Offline install of a pack file
mg lang remove <name>
mg lang update                          Refresh packs to versions compatible with this mg
```

## Behaviour

- `mg project add` scans the repo's file extensions and returns detected languages in its `needs_decision`. The default answer is to install them.
- A file whose language isn't installed renders as plain text, with a hint in the UI ("No Swift highlighting · `mg lang add swift`").
- Adding a language highlights open items in the background. Removing one drops its cache entries.
- `mg doctor` flags packs whose ABI no longer matches after an `mg` upgrade.
