import { createEffect, createMemo, createSignal, For, Show } from "solid-js";
import type { File } from "../api/File";
import type { ItemState } from "../api/ItemState";
import type { Verdict } from "../api/Verdict";
import type { ItemDiff } from "../api/ItemDiff";
import type { MarkViewed } from "../api/MarkViewed";
import { getJson, putJson } from "../app/hub";
import { useKeys } from "../app/keys";
import { inboxHref } from "../app/router";
import { DiffView, type DiffViewControl } from "../diff-view/diff-view";
import { type Row as Line, rowsOf } from "../diff-view/patch";
import { Badge, Empty, Meta, Mono, Row, type Tone } from "../design-system";
import styles from "./item.module.css";
import { ReviewBar } from "./review-bar";

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
  // As the hub moves it: Request changes hands the item back, Comment keeps
  // it with the operator.
  const [moved, setMoved] = createSignal<ItemState>();
  const state = () => moved() ?? props.diff.item.state;
  const reviewed = (verdict: Verdict) => {
    if (verdict === "changes_requested") setMoved("changes_requested");
  };

  // Ids are `<project>#<number>`, and blobs are read per project.
  const project = () => props.diff.item.id.split("#")[0] ?? "";
  const blobUrl = (id: string) =>
    `/api/projects/${encodeURIComponent(project())}/blobs/${id}`;

  // Marks made here, shown before the hub confirms them.
  const [marks, setMarks] = createSignal<ReadonlyMap<string, boolean>>(
    new Map(),
  );
  const [failure, setFailure] = createSignal<string>();
  const isViewed = (file: File) => marks().get(file.path) ?? file.viewed;
  const viewed = createMemo(
    () =>
      new Set(
        props.diff.files.filter(isViewed).map((file) => file.path),
      ) as ReadonlySet<string>,
  );
  const mark = (path: string, value: boolean) => {
    const id = props.diff.item.id;
    const body: MarkViewed = {
      revision: props.diff.item.revision.number,
      path,
      viewed: value,
    };
    setFailure(undefined);
    setMarks((all) => new Map(all).set(path, value));
    putJson(`/api/items/${encodeURIComponent(id)}/viewed`, body).catch(
      (error: unknown) => {
        setMarks((all) => new Map(all).set(path, !value));
        setFailure(error instanceof Error ? error.message : String(error));
      },
    );
  };

  // `j/k` walk every file's lines as one list; a viewed file's are hidden.
  const lines = createMemo(() =>
    props.diff.files.filter((file) => !viewed().has(file.path)).flatMap(rowsOf),
  );
  const order = createMemo(
    () => new Map(props.diff.files.map((file, index) => [file.path, index])),
  );
  const [cursor, setCursor] = createSignal<Line>();
  // The file at the top of the diff, or the one last moved to.
  const [scrolled, setCurrent] = createSignal<string>();
  const current = () => scrolled() ?? props.diff.files[0]?.path;
  let view: DiffViewControl | undefined;

  const moveCursor = (index: number) => {
    const line = lines()[index];
    if (!line) return;
    setCursor(line);
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
    const next = props.diff.files[(order().get(current() ?? "") ?? 0) + step];
    if (next) showFile(next.path);
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
  const stepLine = (step: number) => {
    const at = after(position(), step);
    moveCursor(Math.min(Math.max(at, 0), lines().length - 1));
  };
  const stepHunk = (step: number) => {
    const all = lines();
    for (
      let at = after(position(), step);
      at >= 0 && at < all.length;
      at += step
    ) {
      if (all[at]?.hunkStart) return moveCursor(at);
    }
  };
  const onRow = (row: Line) => {
    const line = lines().find((line) => same(line, row));
    if (line) setCursor(line);
  };
  /** Marking the current file viewed moves on to the next one. */
  const toggleViewed = () => {
    const file = props.diff.files[order().get(current() ?? "") ?? -1];
    if (!file) return;
    const value = !viewed().has(file.path);
    mark(file.path, value);
    if (value && file !== props.diff.files.at(-1)) stepFile(1);
    else showFile(file.path);
  };

  useKeys({
    n: () => stepFile(1),
    p: () => stepFile(-1),
    j: () => stepLine(1),
    k: () => stepLine(-1),
    "]": () => stepHunk(1),
    "[": () => stepHunk(-1),
    v: toggleViewed,
  });

  return (
    <>
      <header class={styles.header}>
        <Mono>{props.diff.item.id}</Mono>
        <h1 class={styles.title}>{props.diff.item.title}</h1>
        <Badge tone="brand">r{props.diff.item.revision.number}</Badge>
        <Badge tone={stateTone[state()]}>{stateLabel[state()]}</Badge>
        <Meta>
          <Mono>{props.diff.item.branch}</Mono> →{" "}
          <Mono>{props.diff.item.target}</Mono>
        </Meta>
        <Counts
          additions={props.diff.item.revision.additions}
          deletions={props.diff.item.revision.deletions}
        />
        <Meta>
          {viewed().size}/{props.diff.files.length} viewed
        </Meta>
        <Show when={failure()}>
          {(message) => <Meta tone="removed">{message()}</Meta>}
        </Show>
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
                <FileEntry
                  file={file}
                  viewed={viewed().has(file.path)}
                  onViewed={(value) => mark(file.path, value)}
                />
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
            viewed={viewed()}
            blobUrl={blobUrl}
            onFile={setCurrent}
            onRow={onRow}
            control={(control) => {
              view = control;
            }}
          />
        </Show>
      </div>
      <Show when={reviewable(state())}>
        <ReviewBar
          item={props.diff.item.id}
          revision={props.diff.item.revision.number}
          onSubmitted={reviewed}
        />
      </Show>
    </>
  );
}

const stateLabel: Record<ItemState, string> = {
  needs_review: "Needs review",
  changes_requested: "Changes requested",
  merging: "Merging",
  merged: "Merged",
  abandoned: "Abandoned",
};

const stateTone: Record<ItemState, Tone> = {
  needs_review: "neutral",
  changes_requested: "warning",
  merging: "brand",
  merged: "success",
  abandoned: "neutral",
};

/** The hub takes a review while the item is with the operator or its agent. */
function reviewable(state: ItemState): boolean {
  return state === "needs_review" || state === "changes_requested";
}

function same(line: Line, other: Line | undefined): boolean {
  return (
    line.path === other?.path &&
    line.side === other.side &&
    line.line === other.line
  );
}

function FileEntry(props: {
  file: File;
  viewed: boolean;
  onViewed: (viewed: boolean) => void;
}) {
  return (
    <>
      <input
        type="checkbox"
        class={styles.viewed}
        checked={props.viewed}
        aria-label={`Viewed ${props.file.path}`}
        // The row would show the file too.
        onClick={(event) => event.stopPropagation()}
        onChange={(event) => {
          props.onViewed(event.currentTarget.checked);
          // A focused field keeps keys from the shortcuts.
          event.currentTarget.blur();
        }}
      />
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
