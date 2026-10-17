//! Language packs: tree-sitter grammars compiled to wasm with their
//! highlight queries, installed by the operator and run in wasmtime, so a
//! grammar is data rather than code built into mg.

mod archive;
mod catalogue;
mod languages;
mod load;
mod manifest;
mod spans;
mod tags;

pub use catalogue::known;
pub use languages::Languages;
