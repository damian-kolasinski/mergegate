import { CodeView, type CodeViewDiffItem } from "@pierre/diffs";
import { createEffect, onCleanup, onMount } from "solid-js";
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
};

/**
 * Every file of a change in one scrolling column, virtualized by CodeView,
 * which owns the element's scrolling.
 */
export function DiffView(props: {
  files: readonly File[];
  /** Where a blob's bytes are, for expanding gaps. */
  blobUrl: (id: string) => string;
  /** The file at the top of the view, as it scrolls. */
  onFile: (path: string) => void;
  /** A line the reviewer clicked. */
  onRow: (row: Row) => void;
  control: (control: DiffViewControl) => void;
}) {
  let root!: HTMLDivElement;
  const byPath = new Map<string, File>();
  const view = new CodeView({
    diffStyle: "unified",
    stickyHeaders: true,
    expansionLineCount: 20,
    themeType: "system",
    enableLineSelection: true,
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
    for (const file of props.files) byPath.set(file.path, file);
    view.setItems(
      props.files.map(
        (file): CodeViewDiffItem => ({
          id: file.path,
          type: "diff",
          fileDiff: toFileDiff(file),
        }),
      ),
    );
  });

  onMount(() => {
    view.setup(root);
    const onScroll = (top: number) => {
      const path = fileAt(top);
      if (path !== undefined) props.onFile(path);
    };
    const unsubscribe = view.subscribeToScroll(onScroll);
    onCleanup(() => {
      unsubscribe();
      view.cleanUp();
    });
    props.control({
      showFile: (path) =>
        view.scrollTo({ type: "item", id: path, behavior: "instant" }),
      showRow: (row) => {
        const range = {
          start: row.line,
          end: row.line,
          side: row.side,
          endSide: row.side,
        };
        view.setSelectedLines({ id: row.path, range });
        view.scrollTo({
          type: "line",
          id: row.path,
          lineNumber: row.line,
          side: row.side,
          align: "nearest",
          behavior: "instant",
        });
      },
      clearRow: () => view.clearSelectedLines(),
    });
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

async function text(url: string): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url} answered ${response.status}`);
  return new TextDecoder().decode(await response.arrayBuffer());
}
