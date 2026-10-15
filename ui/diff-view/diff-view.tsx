import {
  CodeView,
  type CodeViewDiffItem,
  type CodeViewOptions,
  type DiffLineAnnotation,
} from "@pierre/diffs";
import {
  createEffect,
  createSignal,
  type JSX,
  onCleanup,
  onMount,
  Show,
  untrack,
} from "solid-js";
import { render } from "solid-js/web";
import type { BlobHighlights } from "../api/BlobHighlights";
import type { File } from "../api/File";
import { Button } from "../design-system";
import styles from "./diff-view.module.css";
import {
  highlightCSS,
  paintSide,
  paintingOf,
  paintings,
  samePainting,
} from "./highlight";
import { isLfsPointer } from "./lfs";
import { isSubmodule, modeNote } from "./mode";
import { type Row, toFileDiff } from "./patch";
import { previewLabel, previewOf } from "./preview/kind";
import { PreviewDialog } from "./preview/preview";
import previewStyles from "./preview/preview.module.css";

/** What the review screen drives the view with. */
export type DiffViewControl = {
  /** Scrolls `path`'s header to the top. */
  showFile(path: string): void;
  /** Marks `row` as the cursor's line and scrolls it into view. */
  showRow(row: Row): void;
  /** Marks `row` as the cursor's line where it is. */
  selectRow(row: Row): void;
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
  collapsed: ReadonlySet<string>;
  /** Where a blob's bytes are, for expanding gaps and previews. */
  blobUrl: (id: string) => string;
  /** Where a blob's spans are, for expanding gaps. */
  highlightsUrl: (grammar: string, blob: string) => string;
  /** The file at the top of the view, as it scrolls. */
  onFile: (path: string) => void;
  /** A line the reviewer clicked. */
  onRow: (row: Row) => void;
  /** Whether the gutter offers `+`. */
  commentable: boolean;
  /** Lines with something shown under them, which `note` renders. */
  notes: readonly Spot[];
  note: (spot: Spot) => JSX.Element;
  /** Lines the reviewer asked to comment on, from the gutter's `+`. */
  onComment: (span: Span) => void;
  /** Why a file is where it is, shown in its header. */
  describe?: (file: File) => string | undefined;
  control: (control: DiffViewControl) => void;
}) {
  let root!: HTMLDivElement;
  const [previewing, setPreviewing] = createSignal<File>();
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
  let options: CodeViewOptions<string, undefined> = {
    diffStyle: "unified",
    stickyHeaders: true,
    expansionLineCount: 20,
    themeType: "system",
    unsafeCSS: `${highlightCSS}\n${headerCSS}`,
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
        paintBlob(file, "deletions", file.old_blob),
        paintBlob(file, "additions", file.new_blob),
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
      const file = byPath.get(diff.name);
      // A mode changes no line, so a collapsed header still says so.
      const collapsed = items.get(diff.name)?.collapsed;
      const notes = [
        collapsed ? "Viewed" : file && headerStatus(file),
        file && modeNote(file),
        // Backticks would show as typed: the header's text is plain.
        file && props.describe?.(file)?.replaceAll("`", ""),
      ].filter((note): note is string => Boolean(note));
      const preview = file && !collapsed ? previewOf(file) : undefined;
      if (notes.length === 0 && !preview) return undefined;
      const note = document.createElement("span");
      note.className = "mg-header-note";
      notes.forEach((text, index) => {
        if (index > 0) note.append(" · ");
        const part = document.createElement("span");
        part.textContent = text;
        note.append(part);
      });
      note.title = note.textContent;
      if (!file || !preview) return note;
      const element = document.createElement("span");
      mounted.set(
        element,
        render(
          () => (
            <span class={previewStyles.header}>
              {note}
              <Button onClick={() => setPreviewing(file)}>
                {previewLabel(file, preview)}
              </Button>
            </span>
          ),
          element,
        ),
      );
      return element;
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
  };
  const view = new CodeView<string>(options);

  // A held merge takes the item out of review and a cancel brings it back,
  // with the same view on screen. Options are set as a whole.
  createEffect(() => {
    const commentable = props.commentable;
    if (options.enableGutterUtility === commentable) return;
    options = { ...options, enableGutterUtility: commentable };
    view.setOptions(options);
  });

  /** What the header says about a file's diff, beside the reason. */
  const headerStatus = (file: File): string | undefined => {
    const note = contentNote(file);
    if (note !== undefined || file.content.kind !== "text") return note;
    // Said on the first file a pack would colour, not on every one.
    const pack = file.missing_pack;
    if (!pack) return undefined;
    const first = [...byPath.values()].find(
      (other) => other.missing_pack === pack,
    );
    return first === file ? `No ${pack} language pack` : undefined;
  };

  /**
   * Adds a side's spans to `file`'s painting before an expanded gap renders.
   * A blob the hub hasn't highlighted yet shows plain.
   */
  const paintBlob = async (
    file: File,
    side: "deletions" | "additions",
    blob: string,
  ) => {
    if (file.grammar === null) return;
    const response = await fetch(props.highlightsUrl(file.grammar, blob));
    if (!response.ok) return;
    const spans = (await response.json()) as BlobHighlights;
    const painting = paintings.get(file.path);
    if (painting) paintSide(painting, side, spans);
  };

  // The same files again bring new spans, as the hub highlights them, or,
  // compared against another revision, new changes: only the files whose
  // spans or changes differ render again. Any other files rebuild the view.
  createEffect(() => {
    const files = props.files;
    const same =
      files.length === byPath.size &&
      files.every((file) => byPath.has(file.path));
    if (same) {
      for (const file of files) {
        const shown = byPath.get(file.path);
        byPath.set(file.path, file);
        const item = items.get(file.path);
        if (!item || (shown && sameChange(shown, file))) continue;
        paint(file);
        const changed = {
          ...item,
          fileDiff: toFileDiff(file),
          version: (item.version ?? 0) + 1,
        };
        items.set(file.path, changed);
        view.updateItem(changed);
      }
      // A judge's scores re-sort the same files: the view takes them in the
      // new order, each still cached at its version.
      const order = [...items.keys()];
      if (files.some((file, at) => file.path !== order[at])) {
        const sorted = files.flatMap((file) => {
          const item = items.get(file.path);
          return item ? [item] : [];
        });
        items.clear();
        for (const item of sorted) items.set(item.id, item);
        view.setItems(sorted);
      }
      return;
    }
    // The view caches a render by id and version, and a path can come back
    // with other changes, so versions only go up.
    const versions = new Map(
      [...items].map(([path, item]) => [path, item.version ?? 0]),
    );
    byPath.clear();
    items.clear();
    paintings.clear();
    const collapsed = untrack(() => props.collapsed);
    const notes = untrack(() => notesByPath(props.notes));
    for (const file of files) {
      byPath.set(file.path, file);
      paint(file);
      items.set(file.path, {
        id: file.path,
        type: "diff",
        fileDiff: toFileDiff(file),
        collapsed: collapsed.has(file.path),
        annotations: notes.get(file.path) ?? [],
        version: (versions.get(file.path) ?? 0) + 1,
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

  // Only the files that opened or collapsed render again.
  createEffect(() => {
    const collapsed = props.collapsed;
    for (const item of items.values()) {
      if (item.collapsed === collapsed.has(item.id)) continue;
      const changed = {
        ...item,
        collapsed: !item.collapsed,
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
      paintings.clear();
      for (const dispose of mounted.values()) dispose();
      mounted.clear();
    });
    props.control({
      showFile: (path) => {
        movedTo = path;
        view.scrollTo({ type: "item", id: path, behavior: "instant" });
      },
      showRow: (row) => {
        selectRow(row);
        showLine(row);
      },
      selectRow,
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

  const selectRow = (row: Row) => {
    movedTo = row.path;
    const range = {
      start: row.line,
      end: row.line,
      side: row.side,
      endSide: row.side,
    };
    view.setSelectedLines({ id: row.path, range });
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

  return (
    <>
      <div ref={root} class={styles.view} role="region" aria-label="Diff" />
      <Show when={previewing()} keyed>
        {(file) => (
          <PreviewDialog
            file={file}
            blobUrl={props.blobUrl}
            onClose={() => setPreviewing(undefined)}
          />
        )}
      </Show>
    </>
  );
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

/** What `file`'s header says about its diff. */
function contentNote(file: File): string | undefined {
  switch (file.interdiff) {
    case "unseparated":
      return "Can't be told apart from the target's changes; whole diff shown";
    case "dropped":
      return "No longer changed";
  }
  switch (file.content.kind) {
    case "binary":
      // The mode note shows a submodule's commits instead, and an image's
      // button shows the image.
      return isSubmodule(file) || previewOf(file)
        ? undefined
        : "Binary file not shown";
    case "too_large":
      return "Too large to show";
    case "gone":
      return "Diff dropped when the item closed";
  }
  // Only a diff that shows says this: one that can't says why instead.
  switch (file.interdiff) {
    case "added":
      return "Added since the earlier revision";
    case "deleted":
      return "Deleted since the earlier revision";
    default:
      // Text, but not the file itself.
      return isLfsPointer(file)
        ? "Git LFS pointer; the hub doesn't hold the file it points to"
        : undefined;
  }
}

/** The blobs fix a file's hunks; the interdiff and modes, its header. */
function sameChange(a: File, b: File): boolean {
  return (
    a.old_blob === b.old_blob &&
    a.new_blob === b.new_blob &&
    a.old_mode === b.old_mode &&
    a.new_mode === b.new_mode &&
    a.interdiff === b.interdiff &&
    samePainting(a, b)
  );
}

function paint(file: File) {
  const painting = paintingOf(file);
  if (painting) paintings.set(file.path, painting);
  else paintings.delete(file.path);
}

async function text(url: string): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url} answered ${response.status}`);
  return new TextDecoder().decode(await response.arrayBuffer());
}

// The header's counts read `+3 −1` as everywhere else on the page, and a
// long reason gives way to the file's name.
const headerCSS = `
[data-additions-count] { order: -1; }
.mg-header-note {
  max-width: 56ch;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 400;
  opacity: 0.75;
}
`;
