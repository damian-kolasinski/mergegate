import type { File } from "../api/File";
import type { Mode } from "../api/Mode";

const names: Record<Mode, string> = {
  "100644": "file",
  "100755": "executable",
  "120000": "symlink",
  "160000": "submodule",
};

/**
 * What `file`'s modes say that its lines don't: a changed exec bit or type,
 * a symlink, a submodule's commits. Undefined for a plain file, and for one
 * recorded before the hub kept modes.
 */
export function modeNote(file: File): string | undefined {
  const { old_mode: before, new_mode: after } = file;
  if (before === "160000" || after === "160000") return submodule(file);
  if (before !== null && after !== null && before !== after) {
    const modes = `(mode ${before} → ${after})`;
    if (before === "100644" && after === "100755") {
      return `Made executable ${modes}`;
    }
    if (before === "100755" && after === "100644") {
      return `No longer executable ${modes}`;
    }
    return `${capitalized(names[before])} → ${names[after]} ${modes}`;
  }
  const mode = after ?? before;
  if (mode === "120000") return "Symlink";
  if (mode === "100755" && before === null) return "Executable";
  return undefined;
}

/** Whether a side of `file` is a submodule, whose blob is a commit. */
export function isSubmodule(file: File): boolean {
  return file.old_mode === "160000" || file.new_mode === "160000";
}

function submodule(file: File): string {
  const at = (side: "old" | "new") =>
    file[`${side}_mode`] === "160000"
      ? file[`${side}_blob`]?.slice(0, 7)
      : undefined;
  const [before, after] = [at("old"), at("new")];
  if (before && after) return `Submodule ${before} → ${after}`;
  if (after) {
    return file.old_mode === null
      ? `Submodule at ${after}`
      : `${capitalized(names[file.old_mode])} → submodule at ${after}`;
  }
  if (before) {
    return file.new_mode === null
      ? `Submodule removed, was at ${before}`
      : `Submodule at ${before} → ${names[file.new_mode]}`;
  }
  return "Submodule";
}

function capitalized(name: string): string {
  return name.charAt(0).toUpperCase() + name.slice(1);
}
