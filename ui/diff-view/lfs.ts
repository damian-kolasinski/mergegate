import type { File } from "../api/File";

/** A pointer's first line, as the LFS spec fixes it. */
const POINTER = "version https://git-lfs.github.com/spec/v1";

/**
 * Whether `file` is a Git LFS pointer: a few lines of text standing in for
 * a file kept elsewhere, so its diff shows a hash and a size, not the file.
 */
export function isLfsPointer(file: File): boolean {
  if (file.content.kind !== "text") return false;
  const first = file.content.hunks[0];
  if (!first || first.new_start > 1) return false;
  return first.lines.some((line) => line.slice(1) === POINTER);
}
