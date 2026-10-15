import type { Comment } from "../../api/Comment";
import type { Side } from "../../api/Side";
import type { Spot } from "../../diff-view/diff-view";

/**
 * The lines a comment is on in `revision`'s diff: its own on the revision it
 * was written on, or where the hub placed one from an earlier revision. One
 * the hub found outdated, one from a later revision, and one from GitHub on
 * lines no revision's diff shows or on no file have none.
 */
export function linesOn(
  comment: Comment,
  revision: number,
): { start: number; line: number } | undefined {
  if (comment.start_line === null || comment.line === null) return undefined;
  if (comment.revision === revision) {
    return { start: comment.start_line, line: comment.line };
  }
  // The hub places a comment onto the revision the screen asked for, so
  // only an earlier revision's has a placement.
  const placement = comment.placement;
  if (
    comment.revision > revision ||
    placement === undefined ||
    placement.kind === "outdated"
  ) {
    return undefined;
  }
  return { start: placement.start_line, line: placement.line };
}

/** Where `revision`'s diff shows a comment: under its last line. */
export function spotOf(comment: Comment, revision: number): Spot | undefined {
  const lines = linesOn(comment, revision);
  if (comment.path === null || lines === undefined) return undefined;
  return {
    path: comment.path,
    side: comment.side === "old" ? "deletions" : "additions",
    line: lines.line,
  };
}

/**
 * A draft from an earlier revision that a review on `revision` can't take
 * along, as its lines changed or are gone: the operator re-places it or
 * deletes it first.
 */
export function isFlagged(draft: Comment, revision: number): boolean {
  return draft.revision < revision && draft.placement?.kind !== "moved";
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
