// Bundles the UI into one directory the hub serves: `index.html` at its root,
// everything else under `assets/`. The hub's build script runs this with
// `--outdir` in Cargo's OUT_DIR and embeds the result; on its own it writes
// `dist/`, which a hub started with `MG_UI_DIR=ui/dist` serves from disk, so
// `pnpm watch` updates the UI without rebuilding the hub.
//
// File names stay plain (no content hashes): the hub serves them with ETags
// over loopback, and plain names keep the output identical between builds.

import { copyFile, mkdir, rm } from "node:fs/promises";
import { join } from "node:path";
import { parseArgs } from "node:util";
import * as esbuild from "esbuild";
import { solidPlugin } from "esbuild-plugin-solid";

const { values } = parseArgs({
  options: {
    outdir: { type: "string", default: "dist" },
    watch: { type: "boolean", default: false },
  },
});
const outdir = values.outdir;

await rm(outdir, { recursive: true, force: true });
await mkdir(outdir, { recursive: true });
await copyFile("index.html", join(outdir, "index.html"));

// Shiki loads every grammar and theme through a dynamic import, which esbuild
// inlines: 10 MB. The diff view renders plain text with Pierre's two themes
// until highlighting lands, so the rest resolve to a module
// that throws if anything ever asks for one.
const shikiAssets = {
  name: "shiki-assets",
  setup(build) {
    const kept = /^@pierre\/theme\/pierre-(light|dark)$/;
    build.onResolve(
      { filter: /^(@shikijs\/(langs|themes)\/|@pierre\/theme\/|shiki\/wasm$)/ },
      (args) =>
        kept.test(args.path)
          ? undefined
          : { path: args.path, namespace: "shiki-asset" },
    );
    build.onLoad({ filter: /.*/, namespace: "shiki-asset" }, (args) => ({
      contents: `throw new Error(${JSON.stringify(`${args.path} isn't bundled`)});`,
    }));
  },
};

const options = {
  entryPoints: { main: "app/main.tsx" },
  outdir: join(outdir, "assets"),
  bundle: true,
  format: "esm",
  target: "es2022",
  minify: !values.watch,
  sourcemap: values.watch ? "inline" : false,
  entryNames: "[name]",
  assetNames: "[name]",
  loader: { ".woff2": "file" },
  logLevel: "warning",
  plugins: [solidPlugin(), shikiAssets],
};

if (values.watch) {
  const context = await esbuild.context(options);
  await context.watch();
  console.log(`watching, writing to ${outdir}`);
} else {
  await esbuild.build(options);
}
