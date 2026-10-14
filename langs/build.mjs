#!/usr/bin/env node
// Builds language packs from the npm releases pinned in packs.json.
//
//   node langs/build.mjs [--out DIR] [PACK...]
//
// Each pack becomes DIR/<name>-<version>.tar.gz (default DIR: langs/dist),
// holding pack.json and, per grammar, <grammar>.wasm and its concatenated
// <grammar>.{highlights,injections,locals}.scm. Grammars are compiled with
// the pinned tree-sitter CLI rather than taken prebuilt, so every one has the
// ABI of the runtime mg links; the CLI fetches wasi-sdk into ~/.cache/tree-sitter.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const catalogue = JSON.parse(readFileSync(join(here, "packs.json"), "utf8"));
const byName = new Map(catalogue.packs.map((p) => [p.name, p]));

const args = process.argv.slice(2);
let out = join(here, "dist");
const wanted = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--out") out = resolve(args[++i]);
  else wanted.push(args[i]);
}
for (const name of wanted) {
  if (!byName.has(name)) throw new Error(`no pack named ${name} in packs.json`);
}
const packs = wanted.length ? wanted.map((n) => byName.get(n)) : catalogue.packs;

const work = join(here, ".work");
mkdirSync(out, { recursive: true });

for (const pack of packs) {
  const file = await build(pack);
  console.log(file);
}

// "mergegate:<path>" is a query kept in langs/, where upstream's needs a fix.
function queryPath(pkg, file) {
  const own = "mergegate:";
  return file.startsWith(own) ? join(here, file.slice(own.length)) : join(pkg, file);
}

async function build(pack) {
  const root = join(work, pack.name);
  rmSync(root, { recursive: true, force: true });
  // Packages land side by side, so a query path like
  // "../tree-sitter-javascript/queries/highlights.scm" resolves from the
  // package that borrows it.
  const pkg = await unpack(pack, root);
  for (const other of pack.with ?? []) await unpack(byName.get(other), root);

  const stage = join(root, "pack");
  mkdirSync(stage);
  const grammars = [];
  for (const g of pack.grammars) {
    execFileSync(
      "npx",
      ["--yes", `tree-sitter-cli@${catalogue.tree_sitter_cli}`, "build", "--wasm",
        "--output", join(stage, `${g.name}.wasm`), join(pkg, g.path)],
      { stdio: "inherit" },
    );
    for (const kind of ["highlights", "injections", "locals"]) {
      const files = g[kind] ?? [];
      if (!files.length) continue;
      const text = files.map((f) => readFileSync(queryPath(pkg, f), "utf8")).join("\n");
      writeFileSync(join(stage, `${g.name}.${kind}.scm`), text);
    }
    grammars.push({
      name: g.name,
      extensions: g.extensions ?? [],
      filenames: g.filenames ?? [],
      aliases: g.aliases ?? [],
    });
  }
  const manifest = { name: pack.name, version: pack.version, grammars };
  writeFileSync(join(stage, "pack.json"), JSON.stringify(manifest, null, 2) + "\n");

  const file = join(out, `${pack.name}-${pack.version}.tar.gz`);
  // macOS tar otherwise adds `._*` files for extended attributes, which mg
  // refuses as unexpected entries.
  execFileSync("tar", ["czf", file, "-C", stage, "."], {
    env: { ...process.env, COPYFILE_DISABLE: "1" },
  });
  return file;
}

// Downloads a package's tarball, checks it against its pinned integrity and
// extracts it into dir/<package basename>, returning that directory.
async function unpack(pack, dir) {
  const base = pack.npm.split("/").pop();
  const url = `https://registry.npmjs.org/${pack.npm}/-/${base}-${pack.version}.tgz`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: ${res.status}`);
  const bytes = Buffer.from(await res.arrayBuffer());
  const [algo, want] = pack.integrity.split("-", 2);
  const got = createHash(algo).update(bytes).digest("base64");
  if (got !== want) throw new Error(`${url}: integrity ${algo}-${got}, pinned ${pack.integrity}`);

  const target = join(dir, base);
  mkdirSync(target, { recursive: true });
  const tgz = join(dir, `${base}.tgz`);
  writeFileSync(tgz, bytes);
  execFileSync("tar", ["xzf", tgz, "-C", target, "--strip-components", "1"]);
  return target;
}
