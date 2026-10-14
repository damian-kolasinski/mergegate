import { createEffect, createSignal, Show, untrack } from "solid-js";
import type { ItemDiff } from "../api/ItemDiff";
import type { ItemState } from "../api/ItemState";
import type { Review as SentReview } from "../api/Review";
import { getJson } from "../app/hub";
import { useKeys } from "../app/keys";
import { inboxHref } from "../app/router";
import { DiffView, type DiffViewControl } from "../diff-view/diff-view";
import { Empty } from "../design-system";
import { useComments } from "./comments/comments";
import { Thread } from "./comments/thread";
import { FileList } from "./files";
import { ItemHeader } from "./header";
import styles from "./item.module.css";
import { useNavigation } from "./navigation";
import { ReviewBar } from "./review-bar";
import { useViewed } from "./viewed";

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

  return (
    <section class={styles.screen} aria-label={props.id}>
      <Show when={failure()}>{(message) => <Empty>{message()}</Empty>}</Show>
      <Show when={diff()} fallback={<Leave />}>
        {(current) => <Review diff={current()} />}
      </Show>
    </section>
  );
}

/** Escape goes back to the inbox; the review binds its own. */
function Leave() {
  useKeys({ Escape: leave });
  return null;
}

function leave() {
  location.hash = inboxHref;
}

function Review(props: { diff: ItemDiff }) {
  // As the hub moves it: Request changes hands the item back, Comment keeps
  // it with the operator.
  const [moved, setMoved] = createSignal<ItemState>();
  const state = () => moved() ?? props.diff.item.state;
  let view: DiffViewControl | undefined;
  const viewed = useViewed(() => props.diff);
  const nav = useNavigation({
    files: () => props.diff.files,
    viewed: viewed.paths,
    view: () => view,
  });
  // The screen fetches its diff once; comments then change here.
  const comments = useComments(
    untrack(() => props.diff),
    {
      reviewable: () => reviewable(state()),
      reveal: (spot) => view?.showLine(spot),
    },
  );
  const reviewed = (review: SentReview) => {
    if (review.verdict === "changes_requested") setMoved("changes_requested");
    comments.submitted(review);
  };

  // Ids are `<project>#<number>`, and blobs are read per project.
  const project = () => props.diff.item.id.split("#")[0] ?? "";
  const blobUrl = (id: string) =>
    `/api/projects/${encodeURIComponent(project())}/blobs/${id}`;

  /** Marking the current file viewed moves on to the next one. */
  const toggleViewed = () => {
    const file = nav.currentFile();
    if (!file) return;
    const value = !viewed.paths().has(file.path);
    viewed.mark(file.path, value);
    if (value && file !== props.diff.files.at(-1)) nav.stepFile(1);
    else nav.showFile(file.path);
  };
  /** `c` comments on the cursor's line. */
  const comment = () => {
    const line = nav.cursor();
    if (line && reviewable(state())) {
      comments.drafts.start({ ...line, start: line.line });
    }
  };

  // An open comment takes the keys: the editor focuses a few frames after
  // `c`, and what's typed before then mustn't move the cursor away from it.
  const unlessWriting = (action: () => void) => () => {
    if (!comments.editor.open()) action();
  };
  useKeys({
    // Never leaves an open comment behind.
    Escape: () => (comments.editor.open() ? comments.editor.escape() : leave()),
    c: unlessWriting(comment),
    n: unlessWriting(() => nav.stepFile(1)),
    p: unlessWriting(() => nav.stepFile(-1)),
    j: unlessWriting(() => nav.stepLine(1)),
    k: unlessWriting(() => nav.stepLine(-1)),
    "]": unlessWriting(() => nav.stepHunk(1)),
    "[": unlessWriting(() => nav.stepHunk(-1)),
    v: unlessWriting(toggleViewed),
  });

  return (
    <>
      <ItemHeader
        item={props.diff.item}
        state={state()}
        viewed={viewed.paths().size}
        files={props.diff.files.length}
        failure={viewed.failure()}
      />
      <FileList
        files={props.diff.files}
        viewed={viewed.paths()}
        current={nav.current()}
        onShow={nav.showFile}
        onViewed={viewed.mark}
      />
      <div class={styles.diff}>
        <Show
          when={props.diff.files.length}
          fallback={<Empty>This revision changes no files.</Empty>}
        >
          <DiffView
            files={props.diff.files}
            viewed={viewed.paths()}
            blobUrl={blobUrl}
            onFile={nav.scrolledTo}
            onRow={nav.clicked}
            commentable={reviewable(state())}
            notes={comments.threads.spots()}
            note={(spot) => <Thread comments={comments} spot={spot} />}
            onComment={(span) => {
              if (reviewable(state())) comments.drafts.start(span);
            }}
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
          drafts={comments.drafts.list().length}
          replies={comments.replyDrafts.list().length}
          holding={comments.editor.holding()}
          onSubmitted={reviewed}
        />
      </Show>
    </>
  );
}

/** The hub takes a review while the item is with the operator or its agent. */
function reviewable(state: ItemState): boolean {
  return state === "needs_review" || state === "changes_requested";
}
