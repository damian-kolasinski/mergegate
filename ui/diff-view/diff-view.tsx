import {
  CodeView,
  type CodeViewDiffItem,
  type DiffLineAnnotation,
} from "@pierre/diffs";
import { createEffect, type JSX, onCleanup, onMount, untrack } from "solid-js";
import { render } from "solid-js/web";
import type { File } from "../api/File";
import styles from "./diff-view.module.css";
import { type Row, toFileDiff } from "./patch";

/** What the review screen drives the view with. */
export type DiffViewControl = {
  /** Scrolls `path`'s header to the top. */
  showFile(path: string): void;
  /** Marks `row` as the cursor's line and scrolls it into view. */
  showRow(row: Row): void;
  /** Clears the cursor's line, for a file without one. */
  clearRow(): void;
  /** Scrolls `spot` into view, leaving the cursor where it is. */
  showLine(spot: Spot): void;
};

/** A line of one side of a file. */
export type Spot = Pick<Row, "path" | "side" | "line">;

/** Lines `start..=line` of one side of a file. */
export type Span = Spot & { start: number };

/** Where under a line a note sits; one per line, for `note` to fill. */
type Note = DiffLineAnnotation<string>;

/**
 * Every file of a change in one scrolling column, virtualized by CodeView,
 * which owns the element's scrolling.
 */
export function DiffView(props: {
  files: readonly File[];
  /** Paths shown collapsed to their header. */
  viewed: ReadonlySet<string>;
  /** Where a blob's bytes are, for expanding gaps. */
  blobUrl: (id: string) => string;
  /** The file at the top of the view, as it scrolls. */
  onFile: (path: string) => void;
  /** A line the reviewer clicked. */
  onRow: (row: Row) => void;
  /** Whether the gutter offers `+`; read once, as the view is built. */
  commentable: boolean;
  /** Lines with something shown under them, which `note` renders. */
  notes: readonly Spot[];
  note: (spot: Spot) => JSX.Element;
  /** Lines the reviewer asked to comment on, from the gutter's `+`. */
  onComment: (span: Span) => void;
  control: (control: DiffViewControl) => void;
}) {
  let root!: HTMLDivElement;
  const byPath = new Map<string, File>();
  const items = new Map<string, CodeViewDiffItem<string>>();
  // CodeView drops a note's element without saying so, as it re-renders or
  // virtualizes the file away, so roots are disposed once off the page.
  const mounted = new Map<HTMLElement, () => void>();
  const sweep = () => {
    for (const [element, dispose] of mounted) {
      if (element.isConnected) continue;
      mounted.delete(element);
      dispose();
    }
  };
  const view = new CodeView<string>({
    diffStyle: "unified",
    stickyHeaders: true,
    expansionLineCount: 20,
    themeType: "system",
    enableLineSelection: true,
    enableGutterUtility: untrack(() => props.commentable),
    // CodeView pins `+` to the selected lines while there are some, so with
    // the keyboard cursor in a file `+` comments on the cursor's line, as
    // `c` does; elsewhere it follows the pointer.
    onGutterUtilityClick: (range, context) => {
      const side = range.endSide ?? range.side;
      if (!side) return;
      // A drag across both sides comments on its last line only.
      const start =
        (range.side ?? side) === side ? Math.min(range.start, range.end) : 0;
      const line = Math.max(range.start, range.end);
      props.onComment({
        path: context.item.id,
        side,
        line,
        start: start || line,
      });
    },
    renderAnnotation: (note, context) => {
      if (!("side" in note)) return undefined;
      const element = document.createElement("div");
      const spot = {
        path: context.item.id,
        side: note.side,
        line: note.lineNumber,
      };
      mounted.set(
        element,
        render(() => <>{props.note(spot)}</>, element),
      );
      return element;
    },
    loadDiffFiles: async (diff) => {
      const file = byPath.get(diff.name);
      if (!file?.old_blob || !file.new_blob) {
        throw new Error(`${diff.name} has no two sides to expand`);
      }
      const [before, after] = await Promise.all([
        text(props.blobUrl(file.old_blob)),
        text(props.blobUrl(file.new_blob)),
      ]);
      return {
        oldFile: {
          name: file.old_path ?? file.path,
          contents: before,
          lang: "text",
        },
        newFile: { name: file.path, contents: after, lang: "text" },
      };
    },
    renderHeaderMetadata: (diff) => {
      if (items.get(diff.name)?.collapsed) return "Viewed";
      switch (byPath.get(diff.name)?.content.kind) {
        case "binary":
          return "Binary file not shown";
        case "too_large":
          return "Too large to show";
        default:
          return undefined;
      }
    },
    onSelectedLinesChange: (selection) => {
      const side = selection?.range.endSide ?? selection?.range.side;
      if (!selection || !side) return;
      props.onRow({
        path: selection.id,
        side,
        line: selection.range.end,
        hunkStart: false,
      });
    },
  });

  createEffect(() => {
    byPath.clear();
    items.clear();
    const viewed = untrack(() => props.viewed);
    const notes = untrack(() => notesByPath(props.notes));
    for (const file of props.files) {
      byPath.set(file.path, file);
      items.set(file.path, {
        id: file.path,
        type: "diff",
        fileDiff: toFileDiff(file),
        collapsed: viewed.has(file.path),
        annotations: notes.get(file.path) ?? [],
      });
    }
    view.setItems([...items.values()]);
  });

  // Only the files whose notes moved render again; a note's own content
  // follows its signals.
  createEffect(() => {
    const notes = notesByPath(props.notes);
    for (const item of items.values()) {
      const next = notes.get(item.id) ?? [];
      if (sameNotes(item.annotations ?? [], next)) continue;
      const changed = {
        ...item,
        annotations: next,
        version: (item.version ?? 0) + 1,
      };
      items.set(item.id, changed);
      view.updateItem(changed);
    }
  });

  // Only the files whose mark changed render again.
  createEffect(() => {
    const viewed = props.viewed;
    for (const item of items.values()) {
      const collapsed = viewed.has(item.id);
      if (item.collapsed === collapsed) continue;
      const changed = {
        ...item,
        collapsed,
        version: (item.version ?? 0) + 1,
      };
      items.set(item.id, changed);
      view.updateItem(changed);
    }
  });

  onMount(() => {
    view.setup(root);
    const observer = new MutationObserver(sweep);
    observer.observe(root, { childList: true, subtree: true });
    const onScroll = (top: number) => {
      if (movedTo !== undefined && inView(movedTo, top)) return;
      movedTo = undefined;
      const path = fileAt(top);
      if (path !== undefined) props.onFile(path);
    };
    const unsubscribe = view.subscribeToScroll(onScroll);
    onCleanup(() => {
      observer.disconnect();
      unsubscribe();
      view.cleanUp();
      for (const dispose of mounted.values()) dispose();
      mounted.clear();
    });
    props.control({
      showFile: (path) => {
        movedTo = path;
        view.scrollTo({ type: "item", id: path, behavior: "instant" });
      },
      showRow: (row) => {
        movedTo = row.path;
        const range = {
          start: row.line,
          end: row.line,
          side: row.side,
          endSide: row.side,
        };
        view.setSelectedLines({ id: row.path, range });
        showLine(row);
      },
      showLine,
      clearRow: () => view.clearSelectedLines(),
    });
  });

  // The file the operator last moved to, which stays current while any of it
  // is in view. CodeView reports the scroll that showed it a frame later,
  // like one of the operator's, and the file at the top then is often the
  // one before it.
  let movedTo: string | undefined;
  const inView = (path: string, top: number) => {
    const start = view.getTopForItem(path);
    if (start === undefined) return false;
    const next =
      props.files[props.files.findIndex((file) => file.path === path) + 1];
    const end = (next && view.getTopForItem(next.path)) ?? Infinity;
    return start < top + view.getHeight() && end > top;
  };

  const showLine = (spot: Spot) =>
    view.scrollTo({
      type: "line",
      id: spot.path,
      lineNumber: spot.line,
      side: spot.side,
      align: "nearest",
      behavior: "instant",
    });

  /**
   * The last file whose header is at or above `top`. Files at the end may be
   * too short to ever reach the top, so at the bottom the current file stays
   * the one the reviewer last moved to.
   */
  const fileAt = (top: number): string | undefined => {
    const files = props.files;
    if (root.scrollTop + root.clientHeight >= root.scrollHeight - 1) {
      return undefined;
    }
    let low = 0;
    let high = files.length - 1;
    while (low < high) {
      const middle = Math.ceil((low + high) / 2);
      const path = files[middle]?.path ?? "";
      if ((view.getTopForItem(path) ?? Infinity) <= top + 1) low = middle;
      else high = middle - 1;
    }
    return files[low]?.path;
  };

  return <div ref={root} class={styles.view} role="region" aria-label="Diff" />;
}

/**
 * One note per line, in line order. CodeView matches notes to elements by
 * index, so the notes after a new one render again.
 */
function notesByPath(spots: readonly Spot[]): Map<string, Note[]> {
  const byPath = new Map<string, Map<string, Note>>();
  for (const spot of spots) {
    const key = `${spot.side}:${spot.line}`;
    let notes = byPath.get(spot.path);
    if (!notes) byPath.set(spot.path, (notes = new Map()));
    notes.set(key, { side: spot.side, lineNumber: spot.line, metadata: key });
  }
  return new Map(
    [...byPath].map(([path, notes]) => [
      path,
      [...notes.values()].sort(
        (a, b) => a.lineNumber - b.lineNumber || a.side.localeCompare(b.side),
      ),
    ]),
  );
}

function sameNotes(a: readonly Note[], b: readonly Note[]): boolean {
  return (
    a.length === b.length &&
    a.every((note, index) => note.metadata === b[index]?.metadata)
  );
}

async function text(url: string): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url} answered ${response.status}`);
  return new TextDecoder().decode(await response.arrayBuffer());
}
