import { type FileDiffMetadata, processFile } from "@pierre/diffs";
import type { File } from "../api/File";
import type { Hunk } from "../api/Hunk";

/**
 * CodeView's model of `file`, parsed from the hunks the hub computed so the
 * view shows exactly the lines comments anchor to. It's partial: lines
 * between hunks load from the file's blobs when a gap is expanded.
 */
export function toFileDiff(file: File): FileDiffMetadata {
  const diff = parse(hunksOf(file));
  diff.name = file.path;
  if (file.old_path !== null) diff.prevName = file.old_path;
  diff.type = changeOf(file, diff.hunks.length);
  // Plain text until highlighting lands; no grammar is bundled.
  diff.lang = "text";
  return diff;
}

/** A line `j/k` can put the cursor on, numbered on the side it shows. */
export type Row = {
  path: string;
  side: "deletions" | "additions";
  line: number;
  /** Set on a hunk's first line, where `]/[` stop. */
  hunkStart: boolean;
};

/**
 * The cursor's stops in `file`, in display order: each hunk line, a context
 * line numbered on the new side. Lines shown by expanding a gap aren't
 * stops; they aren't part of the change.
 */
export function rowsOf(file: File): Row[] {
  const rows: Row[] = [];
  for (const hunk of hunksOf(file)) {
    let old = hunk.old_start;
    let current = hunk.new_start;
    let hunkStart = true;
    for (const line of hunk.lines) {
      const prefix = line[0];
      if (prefix === "\\") continue;
      if (prefix === "-") {
        rows.push({ path: file.path, side: "deletions", line: old, hunkStart });
        old++;
      } else {
        rows.push({
          path: file.path,
          side: "additions",
          line: current,
          hunkStart,
        });
        current++;
        if (prefix === " ") old++;
      }
      hunkStart = false;
    }
  }
  return rows;
}

function parse(hunks: readonly Hunk[]): FileDiffMetadata {
  // `processFile` rejects a patch without hunks: a binary file, a mode
  // change, a pure rename.
  if (!hunks.length) {
    return {
      name: "",
      type: "change",
      hunks: [],
      splitLineCount: 0,
      unifiedLineCount: 0,
      isPartial: true,
      deletionLines: [],
      additionLines: [],
    };
  }
  // It reads everything before the first `@@ ` as the file's header, where
  // `/dev/null` names leave `name` and `prevName` unset.
  let patch = "--- /dev/null\n+++ /dev/null\n";
  for (const hunk of hunks) {
    patch += `@@ -${hunk.old_start},${hunk.old_lines} +${hunk.new_start},${hunk.new_lines} @@\n`;
    for (const line of hunk.lines) patch += `${line}\n`;
  }
  const diff = processFile(patch, { throwOnError: true });
  if (!diff) throw new Error("no diff parsed");
  return diff;
}

function hunksOf(file: File): readonly Hunk[] {
  return file.content.kind === "text" ? file.content.hunks : [];
}

function changeOf(file: File, hunks: number): FileDiffMetadata["type"] {
  if (file.old_blob === null) return "new";
  if (file.new_blob === null) return "deleted";
  if (file.old_path === null) return "change";
  return hunks === 0 && file.content.kind === "text"
    ? "rename-pure"
    : "rename-changed";
}
