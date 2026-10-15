import {
  CodeView,
  type CodeViewDiffItem,
  type CodeViewOptions,
  type DiffLineAnnotation,
  type FileDiff,
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
import { type Row, shows, toFileDiff } from "./patch";
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

type Drawn = Pick<FileDiff<string>, "setEditorActiveLine">;

/** A file header's Viewed box. */
export type ViewedBox = { checked: boolean; disabledWhy?: string };

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
  /**
   * The Viewed box a file's header shows, as GitHub's does: ticked or not,
   * with why it can't change when it can't. None for a file with no mark.
   */
  viewedBox?: (file: File) => ViewedBox | undefined;
  /** The box ticked or cleared, from it or a click on the header. */
  onViewed?: (path: string, viewed: boolean) => void;
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
    // `+` follows the pointer, except that CodeView pins it to the end of a
    // dragged range.
    onGutterUtilityClick: (range, context) => {
      const side = range.endSide ?? range.side;
      if (!side) return;
      // A drag across both sides comments on its last line only.
      const start =
        (range.side ?? side) === side ? Math.min(range.start, range.end) : 0;
      const line = Math.max(range.start, range.end);
      const span = { path: context.item.id, side, line, start: start || line };
      // The `+` is hidden on lines an expanded gap shows, but a drag from a
      // hunk can still end on one.
      const file = byPath.get(span.path);
      if (file && shows(file, span)) props.onComment(span);
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
      if (!file) return undefined;
      // A mode changes no line, so a collapsed header still says so.
      const collapsed = items.get(diff.name)?.collapsed;
      const notes = [
        collapsed ? undefined : headerStatus(file),
        modeNote(file),
        // Backticks would show as typed: the header's text is plain.
        props.describe?.(file)?.replaceAll("`", ""),
      ].filter((note): note is string => Boolean(note));
      const preview = collapsed ? undefined : previewOf(file);
      // Whether there's a box is the file's to say once; whether it's ticked
      // follows the marks below, as one open for a comment doesn't collapse
      // and render its header again.
      const box = untrack(() => props.viewedBox?.(file));
      if (notes.length === 0 && !preview && !box) return undefined;
      const note = document.createElement("span");
      // Slotted into CodeView's header from this document, so the shadow
      // root's CSS can't reach it.
      note.className = styles.note ?? "";
      notes.forEach((text, index) => {
        if (index > 0) note.append(" · ");
        const part = document.createElement("span");
        part.textContent = text;
        note.append(part);
      });
      note.title = note.textContent;
      const element = document.createElement("span");
      mounted.set(
        element,
        render(
          () => (
            <span class={previewStyles.header}>
              <Show when={notes.length}>{note}</Show>
              <Show when={preview}>
                {(shown) => (
                  <Button onClick={() => setPreviewing(file)}>
                    {previewLabel(file, shown())}
                  </Button>
                )}
              </Show>
              <Show when={box}>
                {(shown) => (
                  <ViewedToggle
                    path={file.path}
                    box={props.viewedBox?.(file) ?? shown()}
                    onChange={(value) => {
                      props.onViewed?.(file.path, value);
                      if (value) keepHeader(file.path);
                    }}
                  />
                )}
              </Show>
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
    // A clicked line is the cursor's, not a selection that would pin `+`.
    onLineSelected: (range) => {
      const single =
        range?.start === range?.end &&
        (range?.endSide ?? range?.side) === range?.side;
      if (range && single) view.clearSelectedLines({ notify: false });
    },
    onPostRender: (_node, instance, phase, context) => {
      const path = context.item.id;
      if (phase === "unmount") {
        if (drawn.get(path) === instance) drawn.delete(path);
        return;
      }
      // CodeView can hand a file's instance on to another file.
      for (const [other, shown] of drawn) {
        if (shown === instance) drawn.delete(other);
      }
      drawn.set(path, instance);
      draw(path);
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

  /**
   * A click on a file's header, outside what's in it to click, flips its
   * Viewed box, as GitHub's header does. The header is in the file's shadow
   * root and the box is slotted into it, so the box is found through its
   * slot.
   */
  const onHeaderClick = (event: MouseEvent) => {
    // A double click is a word picked from the path, and a modified one is
    // a selection extended, not two flips of the mark.
    if (event.button !== 0 || event.defaultPrevented || event.detail > 1)
      return;
    if (event.shiftKey || event.metaKey || event.ctrlKey || event.altKey)
      return;
    const path = event.composedPath();
    const header = path.find(
      (target): target is Element =>
        target instanceof Element && target.hasAttribute("data-diffs-header"),
    );
    if (!header) return;
    const control = path.some(
      (target) =>
        target instanceof Element &&
        target.matches("a, button, input, label, select, textarea"),
    );
    // Selecting the path to copy it isn't a click on the header. Chromium
    // reports a selection inside a shadow root as collapsed on its host, so
    // it's its text that tells.
    if (control || document.getSelection()?.toString()) return;
    const slot = header.querySelector("slot[name=header-metadata]");
    const box = (slot instanceof HTMLSlotElement ? slot.assignedElements() : [])
      .map((element) => element.querySelector("input[data-viewed-box]"))
      .find((input) => input instanceof HTMLInputElement);
    if (!box || box.disabled) return;
    box.click();
  };

  onMount(() => {
    view.setup(root);
    const observer = new MutationObserver(sweep);
    observer.observe(root, { childList: true, subtree: true });
    root.addEventListener("click", onHeaderClick);
    const onScroll = (top: number) => {
      if (movedFrom !== undefined && Math.abs(top - movedFrom) < 1) return;
      movedFrom = undefined;
      if (movedTo !== undefined && inView(movedTo, top)) return;
      movedTo = undefined;
      const path = fileAt(top);
      if (path !== undefined) props.onFile(path);
    };
    const unsubscribe = view.subscribeToScroll(onScroll);
    onCleanup(() => {
      root.removeEventListener("click", onHeaderClick);
      observer.disconnect();
      unsubscribe();
      view.cleanUp();
      paintings.clear();
      for (const dispose of mounted.values()) dispose();
      mounted.clear();
    });
    props.control({
      showFile: (path) => {
        moveTo(path);
        view.scrollTo({ type: "item", id: path, behavior: "instant" });
      },
      showRow: (row) => {
        moveTo(row.path);
        selectRow(row);
        showLine(row);
      },
      selectRow,
      showLine,
      clearRow: () => {
        view.clearSelectedLines({ notify: false });
        setCursor(undefined);
      },
    });
  });

  // The file the operator last moved to, which stays current while any of it
  // is in view. CodeView reports the scroll that showed it a frame later,
  // like one of the operator's, and the file at the top then is often the
  // one before it.
  let movedTo: string | undefined;
  // Where the view was when the operator moved. CodeView scrolls a frame
  // after it's asked, and the browser reports the scroll before that one a
  // frame after it happened, so a move soon after another hears of the
  // first move's scroll before its own lands: that report says where the
  // view still is, not that the operator scrolled away.
  let movedFrom: number | undefined;
  const moveTo = (path: string) => {
    movedTo = path;
    movedFrom = view.getScrollTop();
  };
  const inView = (path: string, top: number) => {
    const start = view.getTopForItem(path);
    if (start === undefined) return false;
    const next =
      props.files[props.files.findIndex((file) => file.path === path) + 1];
    const end = (next && view.getTopForItem(next.path)) ?? Infinity;
    return start < top + view.getHeight() && end > top;
  };

  // The cursor's line, drawn as each file's active line rather than as a
  // selection, which would pin `+` to it away from the pointer.
  let cursor: Spot | undefined;
  // The instance showing each file on the page, to draw the cursor in.
  const drawn = new Map<string, Drawn>();
  const draw = (path: string) => {
    const instance = drawn.get(path);
    const file = byPath.get(path);
    // Another revision can drop the cursor's line, and CodeView throws on a
    // line its diff doesn't have.
    if (
      cursor?.path === path &&
      file &&
      shows(file, { ...cursor, start: cursor.line })
    ) {
      instance?.setEditorActiveLine(cursor.line, { side: cursor.side });
    } else instance?.setEditorActiveLine(null);
  };
  const setCursor = (spot: Spot | undefined) => {
    const was = cursor?.path;
    cursor = spot;
    if (was !== undefined && was !== spot?.path) draw(was);
    if (spot) draw(spot.path);
  };

  /**
   * Keeps a file that collapsed from its sticky header on screen: the
   * scroll was down in its lines, which are gone, so it'd land files below.
   */
  const keepHeader = (path: string) => {
    const top = view.getTopForItem(path);
    if (top === undefined || top >= view.getScrollTop()) return;
    moveTo(path);
    view.scrollTo({ type: "item", id: path, behavior: "instant" });
  };

  const selectRow = (row: Row) => {
    // A dragged range stays while the cursor is on its end, where a click
    // left it; a move takes the cursor, and `+`, off it.
    const selected = view.getSelectedLines();
    const range = selected?.range;
    if (
      range &&
      (selected.id !== row.path ||
        range.end !== row.line ||
        (range.endSide ?? range.side) !== row.side)
    ) {
      view.clearSelectedLines({ notify: false });
    }
    setCursor(row);
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

function ViewedToggle(props: {
  path: string;
  box: ViewedBox;
  onChange: (viewed: boolean) => void;
}) {
  return (
    <label class={styles.viewed} title={props.box.disabledWhy}>
      <input
        type="checkbox"
        data-viewed-box=""
        checked={props.box.checked}
        disabled={props.box.disabledWhy !== undefined}
        aria-label={`Viewed ${props.path}`}
        onChange={(event) => {
          props.onChange(event.currentTarget.checked);
          // A focused field keeps keys from the shortcuts.
          event.currentTarget.blur();
        }}
      />
      Viewed
    </label>
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
  // Only a diff that shows says these: one that can't says why instead.
  const notes = [
    sinceNote(file),
    // Text, but not the file itself.
    isLfsPointer(file)
      ? "Git LFS pointer; the hub doesn't hold the file it points to"
      : undefined,
  ].filter((note) => note !== undefined);
  return notes.length ? notes.join(" · ") : undefined;
}

function sinceNote(file: File): string | undefined {
  switch (file.interdiff) {
    case "added":
      return "Added since the earlier revision";
    case "deleted":
      return "Deleted since the earlier revision";
    case "unchanged":
      return "Unchanged since the earlier revision; its own diff shown";
    default:
      return undefined;
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
// long note gives way to the file's name: the metadata shrinks first, by
// its slotted note, which `styles.note` cuts short. A click on the header
// marks the file viewed. Lines an expanded gap shows aren't part of the
// change, and the hub takes no comment on them. The cursor's line,
// CodeView's active line, which it marks only by its number's colour,
// takes a selected line's tint.
const headerCSS = `
[data-additions-count] { order: -1; }
[data-diffs-header] { cursor: pointer; }
[data-line-type="context-expanded"] [data-gutter-utility-slot] { display: none; }
[data-metadata] { min-width: 0; flex-shrink: 1000; }
::slotted([slot="header-metadata"]) { min-width: 0; overflow: hidden; }
[data-line][data-editor-active-line] {
  --mix-selection-light: 82%;
  --mix-selection-dark: 75%;
}
:is([data-line], [data-column-number])[data-editor-active-line] {
  --diffs-computed-selected-line-bg: light-dark(
    color-mix(in lab, var(--diffs-computed-diff-line-bg) var(--mix-selection-light), var(--diffs-selection-mix-target)),
    color-mix(in lab, var(--diffs-computed-diff-line-bg) var(--mix-selection-dark), var(--diffs-selection-mix-target))
  );
}
`;
