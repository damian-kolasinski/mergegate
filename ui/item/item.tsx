import { createMemo, createSignal, Show, untrack } from "solid-js";
import type { Demo as DemoData } from "../api/Demo";
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
import { Demo } from "./demo/demo";
import { Intent } from "./intent";
import { ItemHeader } from "./header";
import { MergeBlocked } from "./merge-blocked";
import styles from "./item.module.css";
import { useNavigation } from "./navigation";
import { ReviewBar } from "./review-bar";
import { SyncGithub } from "./sync-github";
import { useViewed } from "./viewed";

/**
 * An item's newest revision against its merge-base: every file in one
 * scrolling column, the file list beside it following the scroll.
 */
export function ItemScreen(props: { id: string }) {
  const [diff, setDiff] = createSignal<ItemDiff>();
  // Follows the hub for the revision shown: an agent attaches after it
  // submits, and a strip that waited for a reopen would miss it.
  const [demo, setDemo] = createSignal<DemoData | null>(null);
  const [failure, setFailure] = createSignal<string>();
  // main.tsx keys the route, so the id is fixed for the screen's life.
  const id = untrack(() => props.id);
  const fetchDiff = () =>
    getJson<ItemDiff>(`/api/items/${encodeURIComponent(id)}`);
  const show = (next: ItemDiff) => {
    setDiff(next);
    setDemo(next.demo);
  };
  void fetchDiff().then(show, (error: unknown) =>
    setFailure(error instanceof Error ? error.message : String(error)),
  );
  // A closed item's diffs are gone, so its review gives way to what's left.
  // Other changes, a new revision among them, wait for the operator to
  // reopen the item rather than move the diff under them; the operator's own
  // merge actions `replace` it, since the revision stays and only what's
  // sent and blocked moves.
  const reload = (replace = false) => {
    void fetchDiff().then(
      (next) => {
        const shown = diff()?.item.state;
        if (replace || (closed(next.item.state) && !(shown && closed(shown)))) {
          show(next);
        } else if (next.item.revision.number === diff()?.item.revision.number) {
          setDemo(next.demo);
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
        {(current) => (
          <Review
            diff={current}
            demo={demo()}
            onClosed={() => reload()}
            onMerge={() => reload(true)}
          />
        )}
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

function Review(props: {
  diff: ItemDiff;
  demo: DemoData | null;
  onClosed: () => void;
  onMerge: () => void;
}) {
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
  // The hub highlights in the background, and again as packs come and go;
  // the same revision's files then bring their new spans. The screen keys
  // the review by its diff, so the item is fixed for its life.
  const [files, setFiles] = createSignal(untrack(() => props.diff.files));
  const item = untrack(() => props.diff.item);
  onHubEvent("highlights.changed", () => {
    const revision = item.revision.number;
    void getJson<ItemDiff>(`/api/items/${encodeURIComponent(item.id)}`).then(
      (next) => {
        if (next.item.revision.number === revision) setFiles(next.files);
      },
      () => undefined,
    );
  });
  // Trusted files stay out of the diff until the operator opens their group.
  const review = createMemo(() =>
    files().filter((file) => file.tier === "review"),
  );
  const trusted = createMemo(() =>
    files().filter((file) => file.tier === "trusted"),
  );
  const [showTrusted, setShowTrusted] = createSignal(false);
  const shown = createMemo(() =>
    showTrusted() ? [...review(), ...trusted()] : review(),
  );
  // A trusted file counts as viewed without a mark, but only a mark
  // collapses it, so an opened group shows its diffs.
  const done = createMemo(
    () =>
      new Set([
        ...viewed.paths(),
        ...trusted().map((file) => file.path),
      ]) as ReadonlySet<string>,
  );
  const nav = useNavigation({
    files: shown,
    viewed: viewed.paths,
    view: () => view,
  });
  // The screen fetches its diff once; comments then change here.
  const comments = useComments(
    untrack(() => props.diff),
    {
      reviewable: () => reviewable(state()),
      reveal: (spot) => {
        // A comment on a trusted file opens its group, so the line is there.
        if (trusted().some((file) => file.path === spot.path)) {
          setShowTrusted(true);
        }
        view?.showLine(spot);
      },
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
  const highlightsUrl = (grammar: string, blob: string) =>
    `/api/highlights/${encodeURIComponent(grammar)}/${blob}`;

  /** Marking the current file viewed moves on to the next one. */
  const toggleViewed = () => {
    const file = nav.currentFile();
    if (!file || file.tier === "trusted") return;
    const value = !viewed.paths().has(file.path);
    viewed.mark(file.path, value);
    if (value && file !== shown().at(-1)) nav.stepFile(1);
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
      <div class={styles.top}>
        <ItemHeader
          item={props.diff.item}
          state={state()}
          viewed={done().size}
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
          <Show when={props.diff.item.backend === "github"}>
            <SyncGithub item={props.diff.item.id} />
          </Show>
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
        <Show when={props.diff.guide}>
          {(guide) => <Intent guide={guide()} />}
        </Show>
        <Show when={props.demo}>
          {(demo) => (
            <Demo demo={demo()} revision={props.diff.item.revision.number} />
          )}
        </Show>
        <Show when={props.diff.blocked}>
          {(blocked) => (
            <MergeBlocked
              item={props.diff.item.id}
              target={props.diff.item.target}
              blocked={blocked()}
              onChanged={props.onMerge}
            />
          )}
        </Show>
      </div>
      <FileList
        project={project()}
        files={review()}
        trusted={trusted()}
        expanded={showTrusted()}
        onExpand={(expanded) => {
          const leaving = !expanded && nav.currentFile()?.tier === "trusted";
          setShowTrusted(expanded);
          // The current file mustn't be one the diff no longer shows.
          const last = review().at(-1);
          if (leaving && last) nav.showFile(last.path);
        }}
        viewed={viewed.paths()}
        current={nav.current()}
        onShow={nav.showFile}
        onViewed={viewed.mark}
      />
      <div class={styles.diff}>
        <Show
          when={shown().length}
          fallback={
            <Empty>
              {props.diff.files.length
                ? "Every file in this revision is trusted."
                : "This revision changes no files."}
            </Empty>
          }
        >
          <DiffView
            files={shown()}
            viewed={viewed.paths()}
            blobUrl={blobUrl}
            highlightsUrl={highlightsUrl}
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
          target={props.diff.item.target}
          unviewed={
            props.diff.files.filter((file) => !done().has(file.path)).length
          }
          drafts={comments.drafts.list().length}
          replies={comments.replyDrafts.list().length}
          holding={comments.editor.holding()}
          onSubmitted={reviewed}
          onMerged={props.onMerge}
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
