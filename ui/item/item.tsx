import { createEffect, createMemo, createSignal, For, Show } from "solid-js";
import type { File } from "../api/File";
import type { ItemDiff } from "../api/ItemDiff";
import { getJson } from "../app/hub";
import { useKeys } from "../app/keys";
import { inboxHref } from "../app/router";
import { DiffView, type DiffViewControl } from "../diff-view/diff-view";
import { type Row as Line, rowsOf } from "../diff-view/patch";
import { Badge, Empty, Meta, Mono, Row } from "../design-system";
import styles from "./item.module.css";

/**
 * An item's newest revision against its merge-base: every file in one
 * scrolling column, the file list beside it following the scroll.
 */
export function ItemScreen(props: { id: string }) {
  const [diff, setDiff] = createSignal<ItemDiff>();
  const [failure, setFailure] = createSignal<string>();
  // main.tsx keys the route, so this runs once per screen.
  createEffect(() => {
    void getJson<ItemDiff>(`/api/items/${encodeURIComponent(props.id)}`).then(
      setDiff,
      (error: unknown) =>
        setFailure(error instanceof Error ? error.message : String(error)),
    );
  });

  useKeys({
    Escape: () => {
      location.hash = inboxHref;
    },
  });

  return (
    <section class={styles.screen} aria-label={props.id}>
      <Show when={failure()}>{(message) => <Empty>{message()}</Empty>}</Show>
      <Show when={diff()}>{(current) => <Review diff={current()} />}</Show>
    </section>
  );
}

function Review(props: { diff: ItemDiff }) {
  // Ids are `<project>#<number>`, and blobs are read per project.
  const project = () => props.diff.item.id.split("#")[0] ?? "";
  const blobUrl = (id: string) =>
    `/api/projects/${encodeURIComponent(project())}/blobs/${id}`;

  // `j/k` walk every file's lines as one list.
  const lines = createMemo(() => props.diff.files.flatMap(rowsOf));
  const [cursor, setCursor] = createSignal<number>();
  // The file at the top of the diff, or the one last moved to.
  const [scrolled, setCurrent] = createSignal<string>();
  const current = () => scrolled() ?? props.diff.files[0]?.path;
  let view: DiffViewControl | undefined;

  const moveCursor = (index: number) => {
    const line = lines()[index];
    if (!line) return;
    setCursor(index);
    setCurrent(line.path);
    view?.showRow(line);
  };
  const showFile = (path: string) => {
    setCurrent(path);
    const first = lines().findIndex((line) => line.path === path);
    if (first === -1) {
      setCursor(undefined);
      view?.clearRow();
    } else {
      moveCursor(first);
    }
    view?.showFile(path);
  };
  const stepFile = (step: number) => {
    const files = props.diff.files;
    const index = files.findIndex((file) => file.path === current());
    const next = files[index + step];
    if (next) showFile(next.path);
  };
  /** The cursor, or before the current file's first line when it has none. */
  const position = () =>
    cursor() ?? lines().findIndex((line) => line.path === current()) - 1;
  const stepLine = (step: number) => {
    const at = position();
    moveCursor(Math.min(Math.max(at + step, 0), lines().length - 1));
  };
  const stepHunk = (step: number) => {
    const all = lines();
    for (let at = position() + step; at >= 0 && at < all.length; at += step) {
      if (all[at]?.hunkStart) return moveCursor(at);
    }
  };
  const onRow = (row: Line) => {
    const index = lines().findIndex(
      (line) =>
        line.path === row.path &&
        line.side === row.side &&
        line.line === row.line,
    );
    if (index !== -1) setCursor(index);
  };

  useKeys({
    n: () => stepFile(1),
    p: () => stepFile(-1),
    j: () => stepLine(1),
    k: () => stepLine(-1),
    "]": () => stepHunk(1),
    "[": () => stepHunk(-1),
  });

  return (
    <>
      <header class={styles.header}>
        <Mono>{props.diff.item.id}</Mono>
        <h1 class={styles.title}>{props.diff.item.title}</h1>
        <Badge tone="brand">r{props.diff.item.revision.number}</Badge>
        <Meta>
          <Mono>{props.diff.item.branch}</Mono> →{" "}
          <Mono>{props.diff.item.target}</Mono>
        </Meta>
        <Counts
          additions={props.diff.item.revision.additions}
          deletions={props.diff.item.revision.deletions}
        />
      </header>
      <nav class={styles.files} aria-label="Files">
        <ul>
          <For each={props.diff.files}>
            {(file) => (
              <Row
                selected={file.path === current()}
                onSelect={() => showFile(file.path)}
                onOpen={() => undefined}
              >
                <FileEntry file={file} />
              </Row>
            )}
          </For>
        </ul>
      </nav>
      <div class={styles.diff}>
        <Show
          when={props.diff.files.length}
          fallback={<Empty>This revision changes no files.</Empty>}
        >
          <DiffView
            files={props.diff.files}
            blobUrl={blobUrl}
            onFile={setCurrent}
            onRow={onRow}
            control={(control) => {
              view = control;
            }}
          />
        </Show>
      </div>
    </>
  );
}

function FileEntry(props: { file: File }) {
  return (
    <>
      <span class={styles.path} title={props.file.path}>
        <Mono>{props.file.path}</Mono>
      </span>
      <Counts
        additions={props.file.additions}
        deletions={props.file.deletions}
      />
    </>
  );
}

/** `+N −N`, leaving out a zero or a count a non-text change doesn't have. */
function Counts(props: { additions: number | null; deletions: number | null }) {
  return (
    <span class={styles.counts}>
      <Show when={props.additions}>
        <Meta tone="added">+{props.additions}</Meta>
      </Show>
      <Show when={props.deletions}>
        <Meta tone="removed">−{props.deletions}</Meta>
      </Show>
    </span>
  );
}
