import {
  type ChangeContent,
  type ContextContent,
  cleanLastNewline,
  type FileDiffMetadata,
  type Hunk as PierreHunk,
} from "@pierre/diffs";
import type { File } from "../api/File";
import type { Hunk } from "../api/Hunk";
import { alignedParts, realign } from "./realign";

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
 * line numbered on the new side, a change's lines in the pieces `realign`
 * splits it into. Lines shown by expanding a gap aren't stops; they aren't
 * part of the change.
 */
export function rowsOf(file: File): Row[] {
  const rows: Row[] = [];
  for (const hunk of hunksOf(file)) {
    let old = hunk.old_start;
    let current = hunk.new_start;
    let hunkStart = true;
    let deleted: string[] = [];
    let added: string[] = [];
    const push = (side: Row["side"], line: number) => {
      rows.push({ path: file.path, side, line, hunkStart });
      hunkStart = false;
    };
    const endChange = () => {
      const parts = alignedParts(deleted, added) ?? [
        {
          deletions: deleted.length,
          additions: added.length,
          deletion: 0,
          addition: 0,
        },
      ];
      for (const part of parts) {
        for (let i = 0; i < part.deletions; i++) {
          push("deletions", old + part.deletion + i);
        }
        for (let i = 0; i < part.additions; i++) {
          push("additions", current + part.addition + i);
        }
      }
      old += deleted.length;
      current += added.length;
      deleted = [];
      added = [];
    };
    for (const line of hunk.lines) {
      const prefix = line[0];
      if (prefix === "-") deleted.push(line.slice(1));
      else if (prefix === "+") added.push(line.slice(1));
      else if (prefix === " ") {
        endChange();
        push("additions", current);
        old++;
        current++;
      }
    }
    endChange();
  }
  return rows;
}

/**
 * Builds what `processFile` would parse from the hunks' patch, without
 * writing and reparsing it: for a near-cap file that round trip, which
 * copies every line through a TextEncoder, was a fifth of opening it.
 * Partial, so line indices run on across hunks and the lines between them
 * aren't held.
 */
function parse(hunks: readonly Hunk[]): FileDiffMetadata {
  const diff: FileDiffMetadata = {
    name: "",
    type: "change",
    hunks: [],
    splitLineCount: 0,
    unifiedLineCount: 0,
    isPartial: true,
    deletionLines: [],
    additionLines: [],
  };
  const { deletionLines, additionLines } = diff;
  let lastHunkEnd = 0;
  for (const hunk of hunks) {
    const spec: PierreHunk = {
      collapsedBefore: Math.max(
        startBoundary(hunk.new_start, hunk.new_lines) - lastHunkEnd,
        0,
      ),
      splitLineCount: 0,
      splitLineStart: 0,
      unifiedLineCount: 0,
      unifiedLineStart: 0,
      additionCount: hunk.new_lines,
      additionStart: hunk.new_start,
      additionLines: 0,
      deletionCount: hunk.old_lines,
      deletionStart: hunk.old_start,
      deletionLines: 0,
      deletionLineIndex: deletionLines.length,
      additionLineIndex: additionLines.length,
      hunkContent: [],
      hunkSpecs: `@@ -${hunk.old_start},${hunk.old_lines} +${hunk.new_start},${hunk.new_lines} @@\n`,
      noEOFCRAdditions: false,
      noEOFCRDeletions: false,
    };
    let block: ContextContent | ChangeContent | undefined;
    let last: "context" | "deletion" | "addition" | undefined;
    for (const raw of hunk.lines) {
      const prefix = raw[0];
      if (prefix === "\\") {
        if (!block) continue;
        if (block.type === "context") {
          spec.noEOFCRAdditions = true;
          spec.noEOFCRDeletions = true;
        } else if (last === "deletion") spec.noEOFCRDeletions = true;
        else if (last === "addition") spec.noEOFCRAdditions = true;
        if (last !== "deletion") dropNewline(additionLines);
        if (last !== "addition") dropNewline(deletionLines);
        continue;
      }
      const line = `${raw.slice(1)}\n`;
      if (prefix === " ") {
        if (block?.type !== "context") {
          block = {
            type: "context",
            lines: 0,
            additionLineIndex: additionLines.length,
            deletionLineIndex: deletionLines.length,
          };
          spec.hunkContent.push(block);
        }
        block.lines++;
        deletionLines.push(line);
        additionLines.push(line);
        last = "context";
        continue;
      }
      if (block?.type !== "change") {
        block = {
          type: "change",
          additions: 0,
          deletions: 0,
          additionLineIndex: additionLines.length,
          deletionLineIndex: deletionLines.length,
        };
        spec.hunkContent.push(block);
      }
      if (prefix === "+") {
        block.additions++;
        spec.additionLines++;
        additionLines.push(line);
        last = "addition";
      } else {
        block.deletions++;
        spec.deletionLines++;
        deletionLines.push(line);
        last = "deletion";
      }
    }
    for (const content of spec.hunkContent) {
      if (content.type === "context") {
        spec.splitLineCount += content.lines;
        spec.unifiedLineCount += content.lines;
      } else {
        spec.splitLineCount += Math.max(content.additions, content.deletions);
        spec.unifiedLineCount += content.additions + content.deletions;
      }
    }
    spec.splitLineStart = diff.splitLineCount + spec.collapsedBefore;
    spec.unifiedLineStart = diff.unifiedLineCount + spec.collapsedBefore;
    diff.splitLineCount += spec.collapsedBefore + spec.splitLineCount;
    diff.unifiedLineCount += spec.collapsedBefore + spec.unifiedLineCount;
    diff.hunks.push(spec);
    lastHunkEnd =
      startBoundary(hunk.new_start, hunk.new_lines) + hunk.new_lines;
  }
  realign(diff);
  return diff;
}

/** Where a hunk's side starts, 0-based; an empty side sits after `start`. */
function startBoundary(start: number, count: number) {
  return count === 0 ? start : start - 1;
}

/** For a `\ No newline at end of file` after the last line pushed. */
function dropNewline(lines: string[]) {
  const last = lines.length - 1;
  if (last >= 0) lines[last] = cleanLastNewline(lines[last] ?? "");
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
