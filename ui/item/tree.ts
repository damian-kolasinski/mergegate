import type { File } from "../api/File";

/**
 * The file list as a tree: each module the hub found, then folders in it,
 * then files, a test under the file it covers. Files in no module sit in
 * their own folders at the top. Siblings come in review order, a group
 * where its first file would, so the tree still leads with what to read
 * first.
 */
export type Node = Group | Leaf;

export type Group = {
  kind: "module" | "folder";
  /** Unique in the tree and the same across reloads, as what's collapsed
   * remembers it. */
  key: string;
  /** A module's folder name, or a folder's with the ones it alone holds,
   * as `Scenes/Chat`. */
  name: string;
  /** Where a module lives and the folder all its files share, said once
   * here, as `Products/Messenger · Scenes/`. */
  sub: string;
  nodes: Node[];
  /** The highest judge score in it; null when none was judged. */
  peak: number | null;
  additions: number;
  deletions: number;
  /** Every file in it, so the cursor moving to one can open it. */
  paths: ReadonlySet<string>;
  rank: number;
};

export type Leaf = {
  kind: "file";
  file: File;
  /** The tests of `file` beside it, which follow it. */
  tests: File[];
  /** What else than plain code the file is, as `Tests` or `Interface`,
   * when no file it covers is beside it to say so. */
  tag: string | null;
  rank: number;
};

/** `files` in review order, as a tree whose keys start with `prefix`, so
 * two trees' groups fold apart. */
export function tree(files: File[], prefix: string): Node[] {
  const rank = new Map(files.map((file, index) => [file.path, index]));
  const modules = new Map<string, File[]>();
  const loose: Placed[] = [];
  for (const file of files) {
    if (!file.module) {
      loose.push({ parts: file.path.split("/"), file });
      continue;
    }
    const members = modules.get(file.module.path) ?? [];
    members.push(file);
    modules.set(file.module.path, members);
  }
  const nodes: Node[] = [...modules].map(([path, members]) =>
    module(prefix, path, members, rank),
  );
  nodes.push(...folders(prefix, loose, rank));
  return ordered(nodes);
}

/**
 * `files`, in review order, as `tree` lays them out from top to bottom, a
 * test after the file it covers. The diff follows it so it reads down the
 * list; `tree` of it gives the same tree, as a test's pairing doesn't
 * depend on the order and a group ranks by its first file.
 */
export function treeOrder(files: File[]): File[] {
  return tree(files, "").flatMap(contents);
}

type Placed = { parts: string[]; file: File };

function module(
  prefix: string,
  path: string,
  members: File[],
  rank: ReadonlyMap<string, number>,
): Group {
  const placed = members.map((file) => ({
    parts: (file.module?.inner ?? file.path).split("/"),
    file,
  }));
  // Said once on the module rather than on a folder row of its own.
  const shared = common(placed.map(({ parts }) => parts.slice(0, -1)));
  for (const each of placed) each.parts = each.parts.slice(shared.length);
  const slash = path.lastIndexOf("/");
  const sub = [
    path.slice(0, Math.max(slash, 0)),
    shared.length ? `${shared.join("/")}/` : "",
  ].filter(Boolean);
  return group(
    "module",
    `${prefix}m:${path}`,
    path.slice(slash + 1),
    sub.join(" · "),
    folders(`${prefix}m:${path}/`, placed, rank),
  );
}

/** `placed` laid out by the folders left in their `parts`. */
function folders(
  key: string,
  placed: Placed[],
  rank: ReadonlyMap<string, number>,
): Node[] {
  const here: File[] = [];
  const below = new Map<string, Placed[]>();
  for (const each of placed) {
    const [first, ...rest] = each.parts;
    if (!rest.length || first === undefined) {
      here.push(each.file);
      continue;
    }
    const inside = below.get(first) ?? [];
    inside.push({ parts: rest, file: each.file });
    below.set(first, inside);
  }
  const nodes: Node[] = paired(here, rank);
  for (const [name, inside] of below) {
    let folder = group(
      "folder",
      `${key}${name}/`,
      name,
      "",
      folders(`${key}${name}/`, inside, rank),
    );
    // A folder holding only another says both on one row.
    const [only] = folder.nodes;
    if (folder.nodes.length === 1 && only?.kind === "folder") {
      folder = { ...only, name: `${name}/${only.name}` };
    }
    nodes.push(folder);
  }
  return ordered(nodes);
}

function group(
  kind: Group["kind"],
  key: string,
  name: string,
  sub: string,
  nodes: Node[],
): Group {
  const files = nodes.flatMap(contents);
  const scores = files.flatMap((file) =>
    file.importance === null ? [] : [file.importance],
  );
  return {
    kind,
    key,
    name,
    sub,
    nodes,
    peak: scores.length ? Math.max(...scores) : null,
    additions: files.reduce((sum, file) => sum + (file.additions ?? 0), 0),
    deletions: files.reduce((sum, file) => sum + (file.deletions ?? 0), 0),
    paths: new Set(files.map((file) => file.path)),
    rank: Math.min(...nodes.map((node) => node.rank)),
  };
}

function contents(node: Node): File[] {
  return node.kind === "file"
    ? [node.file, ...node.tests]
    : node.nodes.flatMap(contents);
}

/** One folder's files, each test after the file it covers when that's
 * here too. */
function paired(files: File[], rank: ReadonlyMap<string, number>): Leaf[] {
  const subjects = new Map<string, File[]>();
  for (const file of files) {
    if (isTest(file)) continue;
    const key = stem(name(file));
    subjects.set(key, [...(subjects.get(key) ?? []), file]);
  }
  const tests = new Map<File, File[]>();
  const leaves: Leaf[] = [];
  for (const file of files) {
    // A file in a tests target covers its namesake, as `Login.swift` there
    // covers `Login.swift` in the code's.
    const covered = isTest(file)
      ? (tested(name(file)) ?? stem(name(file)))
      : undefined;
    const subject =
      covered === undefined
        ? undefined
        : subjectOf(file, subjects.get(covered) ?? []);
    if (subject) {
      tests.set(subject, [...(tests.get(subject) ?? []), file]);
      continue;
    }
    leaves.push({
      kind: "file",
      file,
      tests: [],
      tag: file.module?.layer ?? (isTest(file) ? "Tests" : null),
      rank: rank.get(file.path) ?? 0,
    });
  }
  for (const leaf of leaves) {
    leaf.tests = tests.get(leaf.file) ?? [];
    for (const test of leaf.tests) {
      leaf.rank = Math.min(leaf.rank, rank.get(test.path) ?? 0);
    }
  }
  return leaves;
}

/**
 * Which of the files sharing a stem `test` covers: the one of its own kind,
 * as `button.ts` rather than `button.css` for `button.test.ts`, else the
 * first by path. Never by the order they came in, which `treeOrder`
 * changes, so its tree hangs every test where this one did.
 */
function subjectOf(test: File, namesakes: File[]): File | undefined {
  const kind = extension(name(test));
  return [...namesakes].sort(
    (a, b) =>
      Number(extension(name(b)) === kind) -
        Number(extension(name(a)) === kind) || (a.path < b.path ? -1 : 1),
  )[0];
}

function extension(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot) : "";
}

function ordered(nodes: Node[]): Node[] {
  return nodes.sort((a, b) => a.rank - b.rank);
}

function isTest(file: File): boolean {
  return file.module?.layer === "Tests" || tested(name(file)) !== undefined;
}

/**
 * The stem of the file a test named `name` covers, as `Login` for
 * `LoginTests.swift`, `Login.tests.swift`, `login_tests.rs`, `login.spec.ts`
 * or `test_login.py`; undefined for a name no test has.
 */
function tested(name: string): string | undefined {
  const base = stem(name);
  return (
    /^(.+?)(?:[._-](?:tests?|spec)|Tests?|Spec)$/.exec(base)?.[1] ??
    /^test_(.+)$/.exec(base)?.[1]
  );
}

function name(file: File): string {
  return file.path.slice(file.path.lastIndexOf("/") + 1);
}

function stem(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

/** The folders every one of `folders` starts with. */
function common(folders: string[][]): string[] {
  const [first, ...rest] = folders;
  if (!first) return [];
  let depth = first.length;
  for (const folder of rest) {
    let same = 0;
    while (same < depth && folder[same] === first[same]) same++;
    depth = same;
  }
  return first.slice(0, depth);
}
