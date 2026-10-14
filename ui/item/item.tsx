import { createSignal, Show, untrack } from "solid-js";
import type { ItemDiff } from "../api/ItemDiff";
import type { ItemState } from "../api/ItemState";
import type { Review as SentReview } from "../api/Review";
import { getJson, onHubEvent } from "../app/hub";
import { useKeys } from "../app/keys";
import { inboxHref } from "../app/router";
import { DiffView, type DiffViewControl } from "../diff-view/diff-view";
import { Empty } from "../design-system";
import { useComments } from "./comments/comments";
import { Conversation } from "./conversation";
import { Thread } from "./comments/thread";
import { FileList } from "./files";
import { Abandon } from "./abandon";
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
  // main.tsx keys the route, so the id is fixed for the screen's life.
  const id = untrack(() => props.id);
  const fetchDiff = () =>
    getJson<ItemDiff>(`/api/items/${encodeURIComponent(id)}`);
  void fetchDiff().then(setDiff, (error: unknown) =>
    setFailure(error instanceof Error ? error.message : String(error)),
  );
  // A closed item's diffs are gone, so its review gives way to what's left.
  // Other changes, a new revision among them, wait for the operator to
  // reopen the item rather than move the diff under them.
  const reload = () => {
    void fetchDiff().then(
      (next) => {
        const shown = diff()?.item.state;
        if (closed(next.item.state) && !(shown && closed(shown))) {
          setDiff(next);
        }
      },
      (error: unknown) =>
        setFailure(error instanceof Error ? error.message : String(error)),
    );
  };
  onHubEvent("item.changed", (event) => {
    if (!event || event.item === id) reload();
  });

  return (
    <section class={styles.screen} aria-label={props.id}>
      <Show when={failure()}>{(message) => <Empty>{message()}</Empty>}</Show>
      {/* Keyed: the diff only changes when the item closes, and the review
          starts over on what is left. */}
      <Show when={diff()} keyed fallback={<Leave />}>
        {(current) => <Review diff={current} onClosed={reload} />}
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

function Review(props: { diff: ItemDiff; onClosed: () => void }) {
  // As the hub moves it: Request changes hands the item back, Comment keeps
  // it with the operator.
  const [moved, setMoved] = createSignal<ItemState>();
  const state = () => moved() ?? props.diff.item.state;
  // Every comment on the item, the older revisions' too, over the diff.
  const [talking, setTalking] = createSignal(false);
  const [abandoning, setAbandoning] = createSignal(false);
  let toggle: HTMLButtonElement | undefined;
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
      showAll: () => setTalking(true),
    },
  );
  /** Closes the list, unless that would hide what's typed in it. */
  const hideAll = (): boolean => {
    if (comments.aside()) {
      comments.editor.escape();
      return false;
    }
    setTalking(false);
    return true;
  };
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
    Escape: () => {
      if (comments.editor.open()) comments.editor.escape();
      else if (abandoning()) setAbandoning(false);
      // Focus was likely in the list, which takes it along when it closes.
      else if (talking()) {
        if (hideAll()) toggle?.focus();
      } else leave();
    },
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
      >
        <Conversation
          comments={comments}
          open={talking()}
          onToggle={() => (talking() ? hideAll() : setTalking(true))}
          onShow={hideAll}
          ref={(element) => {
            toggle = element;
          }}
        />
        <Show when={open(state())}>
          <Abandon
            item={props.diff.item.id}
            open={abandoning()}
            onToggle={() => setAbandoning(!abandoning())}
            onAbandoned={(item) => {
              setAbandoning(false);
              setMoved(item.state);
              props.onClosed();
            }}
          />
        </Show>
      </ItemHeader>
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

/** Merged and abandoned items stay closed. */
function closed(state: ItemState): boolean {
  return state === "merged" || state === "abandoned";
}

function open(state: ItemState): boolean {
  return !closed(state);
}
