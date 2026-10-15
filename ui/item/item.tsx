import { createMemo, createSignal, Show, untrack } from "solid-js";
import type { Demo as DemoData } from "../api/Demo";
import type { Item } from "../api/Item";
import type { ItemDiff } from "../api/ItemDiff";
import type { ItemState } from "../api/ItemState";
import type { Review as SentReview } from "../api/Review";
import { getJson, onHubEvent } from "../app/hub";
import { useKeys } from "../app/keys";
import { inboxHref } from "../app/router";
import { DiffView, type DiffViewControl } from "../diff-view/diff-view";
import { Empty } from "../design-system";
import { useComments } from "./comments/comments";
import { ComparePicker } from "./compare";
import { Conversation } from "./conversation";
import { Thread } from "./comments/thread";
import { FileList } from "./files";
import { Abandon } from "./abandon";
import { Demo } from "./demo/demo";
import { Intent } from "./intent";
import { ItemHeader } from "./header";
import { MergeBlocked } from "./merge-blocked";
import { NewerRevision } from "./newer-revision";
import { PriorityPicker } from "../priority/priority";
import styles from "./item.module.css";
import { useNavigation } from "./navigation";
import { ReviewBar } from "./review-bar";
import { SyncGithub } from "./sync-github";
import { useViewed } from "./viewed";

/**
 * An item's newest revision against its merge-base or an earlier revision:
 * every file in one scrolling column, the file list beside it following the
 * scroll.
 */
export function ItemScreen(props: { id: string }) {
  const [diff, setDiff] = createSignal<ItemDiff>();
  // Follows the hub for the revision shown: an agent attaches after it
  // submits, and a strip that waited for a reopen would miss it.
  const [demo, setDemo] = createSignal<DemoData | null>(null);
  const [failure, setFailure] = createSignal<string>();
  // A revision submitted after the one on screen.
  const [newest, setNewest] = createSignal<number>();
  // The review bar's summary, kept here so switching to a newer revision
  // doesn't lose what's typed.
  const [summary, setSummary] = createSignal("");
  // The review starts over on a new revision and on what's left of a closed
  // item; comparing against something else only changes the files shown.
  const shownKey = createMemo(() => {
    const shown = diff();
    return shown && `${shown.item.revision.number} ${closed(shown.item.state)}`;
  });
  // The item as the hub last gave it, for what changes without a new diff:
  // its priority and note.
  const [latest, setLatest] = createSignal<Item>();
  // main.tsx keys the route, so the id is fixed for the screen's life.
  const id = untrack(() => props.id);
  // What the operator picked to compare against: `all` or a revision. Until
  // they pick, the hub chooses.
  let since: string | undefined;
  const fetchDiff = (compare = since) =>
    getJson<ItemDiff>(
      `/api/items/${encodeURIComponent(id)}` +
        (compare ? `?since=${encodeURIComponent(compare)}` : ""),
    );
  const failed = (error: unknown) =>
    setFailure(error instanceof Error ? error.message : String(error));
  const show = (next: ItemDiff) => {
    setDiff(next);
    setDemo(next.demo);
    setLatest(next.item);
  };
  void fetchDiff().then(show, failed);
  // A closed item's diffs are gone, so its review gives way to what's left.
  // A new revision waits for the operator to switch to it rather than move
  // the diff under them; the operator's own merge actions `replace` it,
  // since the revision stays and only what's sent and blocked moves.
  const reload = (replace = false) => {
    void fetchDiff().then((next) => {
      setLatest(next.item);
      const shown = diff();
      if (
        replace ||
        (closed(next.item.state) && !(shown && closed(shown.item.state)))
      ) {
        show(next);
        setNewest(undefined);
      } else if (
        shown &&
        next.item.revision.number > shown.item.revision.number
      ) {
        setNewest(next.item.revision.number);
      } else if (next.item.revision.number === shown?.item.revision.number) {
        setDemo(next.demo);
      }
    }, failed);
  };
  /** Shows the newest revision against `compare`: `all` or a revision. */
  const compare = (compare: string) => {
    since = compare;
    void fetchDiff().then((next) => {
      show(next);
      setNewest(undefined);
    }, failed);
  };
  onHubEvent("item.changed", (event) => {
    if (!event || event.item === id) reload();
  });
  // The hub highlights in the background, and again as packs come and go;
  // the diff on screen then brings its new spans.
  onHubEvent("highlights.changed", () => {
    const shown = untrack(diff);
    if (!shown) return;
    void fetchDiff(shown.since === null ? "all" : String(shown.since)).then(
      (next) => {
        // Unless the operator compared otherwise meanwhile, or the hub
        // answered with a newer revision.
        const same = next.item.revision.number === shown.item.revision.number;
        if (diff() === shown && same) setDiff(next);
      },
      () => undefined,
    );
  });

  return (
    <section class={styles.screen} aria-label={props.id}>
      <Show when={failure()}>{(message) => <Empty>{message()}</Empty>}</Show>
      <Show when={shownKey()} keyed fallback={<Leave />}>
        <Show when={diff()}>
          {(current) => (
            <Review
              diff={current()}
              latest={latest() ?? current().item}
              demo={demo()}
              newest={newest()}
              summary={summary()}
              onSummary={setSummary}
              onCompare={compare}
              onPrioritized={setLatest}
              onClosed={() => reload()}
              onMerge={() => reload(true)}
            />
          )}
        </Show>
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
  latest: Item;
  demo: DemoData | null;
  /** A revision newer than the one on screen. */
  newest: number | undefined;
  summary: string;
  onSummary: (summary: string) => void;
  onCompare: (since: string) => void;
  onPrioritized: (item: Item) => void;
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
  const [picking, setPicking] = createSignal(false);
  const priority = () => props.latest.priority;
  const note = () => props.latest.note;
  let toggle: HTMLButtonElement | undefined;
  let view: DiffViewControl | undefined;
  const viewed = useViewed(() => props.diff);
  // Since an earlier revision, files unchanged since then are left out of
  // the diff, and dropped ones are in it but not the revision's own.
  const changed = createMemo(() =>
    props.diff.files.filter((file) => file.interdiff !== "unchanged"),
  );
  const unchanged = createMemo(() =>
    props.diff.files.filter((file) => file.interdiff === "unchanged"),
  );
  const own = createMemo(() =>
    props.diff.files.filter((file) => file.interdiff !== "dropped"),
  );
  // Trusted files stay out of the diff until the operator opens their group.
  const review = createMemo(() =>
    changed().filter((file) => file.tier === "review"),
  );
  const trusted = createMemo(() =>
    changed().filter((file) => file.tier === "trusted"),
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
        ...own()
          .filter((file) => file.tier === "trusted")
          .map((file) => file.path),
      ]) as ReadonlySet<string>,
  );
  // The old side of an interdiff is the earlier revision replayed onto the
  // newest's base, which no revision has, so only the new side takes
  // comments, and a dropped file isn't the newest's to comment on.
  const onOwnSide = (spot: { path: string; side: string }) =>
    props.diff.since === null ||
    (spot.side === "additions" &&
      own().some((file) => file.path === spot.path));
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
    // The newest revision doesn't change a dropped file, so it has no mark.
    if (file.interdiff === "dropped") {
      nav.stepFile(1);
      return;
    }
    const value = !viewed.paths().has(file.path);
    viewed.mark(file.path, value);
    if (value && file !== shown().at(-1)) nav.stepFile(1);
    else nav.showFile(file.path);
  };
  /** `c` comments on the cursor's line. */
  const comment = () => {
    const line = nav.cursor();
    if (line && reviewable(state()) && onOwnSide(line)) {
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
    // `p` is the previous file here.
    P: unlessWriting(() => {
      if (open(state())) setPicking(true);
    }),
  });

  return (
    <>
      <div class={styles.top}>
        <ItemHeader
          item={props.diff.item}
          state={state()}
          priority={priority()}
          note={note()}
          viewed={done().size}
          files={own().length}
          failure={viewed.failure()}
        >
          <ComparePicker
            since={props.diff.since}
            interdiffs={props.diff.interdiffs}
            holding={comments.editor.holding()}
            onChange={props.onCompare}
          />
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
        <Show when={picking()}>
          <PriorityPicker
            item={props.diff.item.id}
            priority={priority()}
            note={note()}
            onClose={() => setPicking(false)}
            onSaved={(item) => {
              setPicking(false);
              props.onPrioritized(item);
            }}
          />
        </Show>
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
        <Show when={props.newest}>
          {(newest) => (
            <NewerRevision
              newest={newest()}
              shown={props.diff.item.revision.number}
              holding={comments.editor.holding()}
              onSwitch={() =>
                props.onCompare(String(props.diff.item.revision.number))
              }
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
        unchanged={unchanged()}
        since={props.diff.since}
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
              {props.diff.since === null
                ? props.diff.files.length
                  ? "Every file in this revision is trusted."
                  : "This revision changes no files."
                : changed().length
                  ? `Every file changed since r${props.diff.since} is trusted.`
                  : `Nothing changed since r${props.diff.since}.`}
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
            notes={comments.threads.spots().filter(onOwnSide)}
            note={(spot) => <Thread comments={comments} spot={spot} />}
            onComment={(span) => {
              if (reviewable(state()) && onOwnSide(span)) {
                comments.drafts.start(span);
              }
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
          behind={props.newest !== undefined}
          unviewed={own().filter((file) => !done().has(file.path)).length}
          drafts={comments.drafts.list().length}
          replies={comments.replyDrafts.list().length}
          holding={comments.editor.holding()}
          summary={props.summary}
          onSummary={props.onSummary}
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
