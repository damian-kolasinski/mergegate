import type { Comment } from "../../api/Comment";
import type { Side } from "../../api/Side";
import type { Spot } from "../../diff-view/diff-view";

/** Where the diff shows a comment: under its last line. */
export function spotOf(comment: Comment): Spot {
  return {
    path: comment.path,
    side: comment.side === "old" ? "deletions" : "additions",
    line: comment.line,
  };
}

export const sideOf: Record<Spot["side"], Side> = {
  deletions: "old",
  additions: "new",
};

export function sameSpot(a: Spot, b: Spot): boolean {
  return a.path === b.path && a.side === b.side && a.line === b.line;
}

export function linesOf(start: number, line: number): string {
  return start === line ? `Line ${line}` : `Lines ${start}–${line}`;
}
