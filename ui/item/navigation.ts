import { createMemo, createSignal } from "solid-js";
import type { File } from "../api/File";
import type { DiffViewControl } from "../diff-view/diff-view";
import { type Row as Line, rowsOf } from "../diff-view/patch";

/**
 * Where the operator is in the diff: the current file, and the line cursor
 * that `j/k` walk over every file's lines as one list, a viewed file's
 * hidden.
 */
export function useNavigation(options: {
  files: () => File[];
  viewed: () => ReadonlySet<string>;
  view: () => DiffViewControl | undefined;
}) {
  const lines = createMemo(() =>
    options
      .files()
      .filter((file) => !options.viewed().has(file.path))
      .flatMap(rowsOf),
  );
  const order = createMemo(
    () => new Map(options.files().map((file, index) => [file.path, index])),
  );
  const [cursor, setCursor] = createSignal<Line>();
  // The file at the top of the diff, or the one last moved to. Comparing
  // another way or a refresh can stop showing it, and then the first file
  // shown is current.
  const [scrolled, setCurrent] = createSignal<string>();
  const current = () => {
    const path = scrolled();
    return path !== undefined && order().has(path)
      ? path
      : options.files()[0]?.path;
  };

  const moveCursor = (index: number) => {
    const line = lines()[index];
    if (!line) return;
    setCursor(line);
    setCurrent(line.path);
    options.view()?.showRow(line);
  };
  const showFile = (path: string) => {
    setCurrent(path);
    const first = lines().find((line) => line.path === path);
    setCursor(first);
    // Not scrolled to: the file's header goes to the top, its first line
    // with it, and each scroll costs the view a layout and a render.
    if (first) options.view()?.selectRow(first);
    else options.view()?.clearRow();
    options.view()?.showFile(path);
  };
  /**
   * The cursor's index in `lines`, or a half step before the first line at
   * or after the current file when the cursor is on none of them.
   */
  const position = () => {
    const all = lines();
    const at = all.findIndex((line) => same(line, cursor()));
    if (at !== -1) return at;
    const from = order().get(current() ?? "") ?? 0;
    const next = all.findIndex((line) => (order().get(line.path) ?? 0) >= from);
    return (next === -1 ? all.length : next) - 0.5;
  };
  const after = (at: number, step: number) =>
    step > 0 ? Math.floor(at) + 1 : Math.ceil(at) - 1;

  return {
    cursor,
    current,
    currentFile: () => options.files()[order().get(current() ?? "") ?? -1],
    showFile,
    stepFile(step: number) {
      const next = options.files()[(order().get(current() ?? "") ?? 0) + step];
      if (next) showFile(next.path);
    },
    stepLine(step: number) {
      const at = after(position(), step);
      moveCursor(Math.min(Math.max(at, 0), lines().length - 1));
    },
    stepHunk(step: number) {
      const all = lines();
      for (
        let at = after(position(), step);
        at >= 0 && at < all.length;
        at += step
      ) {
        if (all[at]?.hunkStart) return moveCursor(at);
      }
    },
    /** The diff scrolled another file to the top. */
    scrolledTo: setCurrent,
    /** The operator clicked a line. */
    clicked(row: Line) {
      const line = lines().find((line) => same(line, row));
      if (line) setCursor(line);
    },
  };
}

function same(line: Line, other: Line | undefined): boolean {
  return (
    line.path === other?.path &&
    line.side === other.side &&
    line.line === other.line
  );
}
