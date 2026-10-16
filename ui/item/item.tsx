import {
  createEffect,
  createMemo,
  createSignal,
  on,
  onCleanup,
  Show,
  untrack,
} from "solid-js";
import type { Demo as DemoData } from "../api/Demo";
import type { Item } from "../api/Item";
import type { ItemDiff } from "../api/ItemDiff";
import type { ItemState } from "../api/ItemState";
import type { Review as SentReview } from "../api/Review";
import { type Failure, failureOf, getJson, onHubEvent } from "../app/hub";
import { useKeys } from "../app/keys";
import { learnName } from "../app/names";
import { go, inboxHref, itemHref } from "../app/router";
import { useTitle } from "../app/title";
import {
  DiffView,
  type DiffViewControl,
  type Span,
} from "../diff-view/diff-view";
import { Button, Empty, ErrorNote } from "../design-system";
import { useComments } from "./comments/comments";
import { DeletedDraft } from "./comments/drafts";
import { ComparePicker } from "./compare";
import { Conversation } from "./conversation";
import { Thread } from "./comments/thread";
import { FileList } from "./files";
import { Abandon } from "./abandon";
import { Closed } from "./closed";
import { Demo } from "./demo/demo";
import { Intent } from "./intent";
import { ItemHeader } from "./header";
import { MergeBlocked } from "./merge-blocked";
import { MoreMenu } from "./more";
import { NewerRevision } from "./newer-revision";
import { PriorityPicker } from "../priority/priority";
import styles from "./item.module.css";
import { useNavigation } from "./navigation";
import { ReviewBar } from "./review-bar";
import { SyncGithub } from "./sync-github";
import { useViewed } from "./viewed";
import { describe } from "./why";

/**
 * An item's newest revision against its merge-base or an earlier revision:
 * every file in one scrolling column, the file list beside it following the
 * scroll.
 */
export function ItemScreen(props: { id: string }) {
  const [diff, setDiff] = createSignal<ItemDiff>();
  // Follows the hub for the revision shown: an agent attaches after it
  // submits, and a chip that waited for a reopen would miss it.
  const [demo, setDemo] = createSignal<DemoData | null>(null);
  const [failure, setFailure] = createSignal<Failure>();
  // A revision submitted after the one on screen.
  const [newest, setNewest] = createSignal<number>();
  // main.tsx keys the route, so the id is fixed for the screen's life.
  const id = untrack(() => props.id);
  // What's typed for the review and for Abandon, kept here so switching to
  // a newer revision doesn't lose it, and in the tab's session so leaving
  // the item or reloading doesn't either.
  const kept = readDraft(id);
  const [summary, setSummary] = createSignal(kept.summary);
  const [reason, setReason] = createSignal(kept.reason);
  createEffect(() => writeDraft(id, { summary: summary(), reason: reason() }));
  // The review starts over on a new revision and on what's left of a closed
  // item; comparing against something else only changes the files shown.
  const shownKey = createMemo(() => {
    const shown = diff();
    return shown && `${shown.item.revision.number} ${closed(shown.item.state)}`;
  });
  // The item as the hub last gave it, for what changes without a new diff:
  // its state, priority and note.
  const [latest, setLatest] = createSignal<Item>();
  // The open items the revision on screen conflicts with, kept apart from
  // the diff so the watch finding or dropping one doesn't re-render it.
  const [conflicts, setConflicts] = createSignal<readonly string[]>([], {
    equals: (a, b) => a.join() === b.join(),
  });
  createEffect(() => {
    const item = latest();
    if (item) learnName(item.id, item.name);
  });
  useTitle(() => {
    const item = latest();
    return item ? `${item.name} ${item.title}` : id;
  });
  // What the operator picked to compare against: `all` or a revision. Until
  // they pick, the hub shows all changes.
  let since: string | undefined;
  const fetchDiff = (compare = since) =>
    getJson<ItemDiff>(
      `/api/items/${encodeURIComponent(id)}` +
        (compare ? `?since=${encodeURIComponent(compare)}` : ""),
    );
  const failed = (error: unknown) => setFailure(failureOf(error));
  // The hub fills in the diff on screen after it arrives: the highlighter
  // its spans, the judge its scores. New files re-render the diff, and every
  // (re)connect refetches, so only a real change replaces them; nor does a
  // diff the operator compared away from, or of another revision. It's
  // checked against the diff on screen when the answer arrives, which an
  // earlier answer may have filled in since this one was asked for.
  const follow = (next: ItemDiff) => {
    const shown = diff();
    if (
      shown &&
      next.item.revision.number === shown.item.revision.number &&
      next.since === shown.since &&
      JSON.stringify(next.files) !== JSON.stringify(shown.files)
    ) {
      setDiff(next);
    }
  };
  const show = (next: ItemDiff) => {
    setFailure(undefined);
    setDiff(next);
    setDemo(next.demo);
    setLatest(next.item);
    setConflicts(next.conflicts_with);
  };
  // A highlights refetch answered before the first diff, which it's newer
  // than: the hub may have served the first one before it had the spans.
  let early: ItemDiff | undefined;
  void fetchDiff().then((next) => {
    show(next);
    if (early) follow(early);
    early = undefined;
  }, failed);
  // A closed item's diffs are gone, so its review gives way to what's left.
  // A new revision waits for the operator to switch to it rather than move
  // the diff under them; the operator's own merge actions `replace` it,
  // since the revision stays and only what's sent and blocked moves.
  const reload = (replace = false) => {
    void fetchDiff().then((next) => {
      setFailure(undefined);
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
        setConflicts(next.conflicts_with);
        follow(next);
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
  // The hub highlights in the background, and again as packs come and go,
  // saying so each time; the refetches are numbered so an answer arriving
  // after a newer one's doesn't put older spans back.
  let asked = 0;
  let answered = 0;
  onHubEvent("highlights.changed", (event) => {
    const shown = untrack(diff);
    // The stream opens while a large first diff is still on its way, and
    // asking for it again then would slow it down.
    if (!shown && !event) return;
    const number = ++asked;
    const compared = shown && (shown.since === null ? "all" : `${shown.since}`);
    void fetchDiff(compared).then(
      (next) => {
        if (number < answered) return;
        answered = number;
        if (diff()) follow(next);
        else early = next;
      },
      () => undefined,
    );
  });

  return (
    <section class={styles.screen} aria-label={props.id}>
      <Show when={!diff() && failure()}>
        {(current) => (
          <Empty>
            <ErrorNote failure={current()} />
          </Empty>
        )}
      </Show>
      <Show when={shownKey()} keyed fallback={<Leave />}>
        <Show when={diff()}>
          {(current) => (
            <Review
              diff={current()}
              latest={latest() ?? current().item}
              conflictsWith={conflicts()}
              demo={demo()}
              newest={newest()}
              failure={failure()}
              onRetry={() => reload()}
              summary={summary()}
              onSummary={setSummary}
              reason={reason()}
              onReason={setReason}
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
  go(inboxHref);
}

function Review(props: {
  diff: ItemDiff;
  latest: Item;
  /** The open items the revision on screen conflicts with. */
  conflictsWith: readonly string[];
  demo: DemoData | null;
  /** A revision newer than the one on screen. */
  newest: number | undefined;
  /** The hub refused to say how the item stands now. */
  failure: Failure | undefined;
  onRetry: () => void;
  summary: string;
  onSummary: (summary: string) => void;
  /** What's typed as the reason to abandon. */
  reason: string;
  onReason: (reason: string) => void;
  onCompare: (since: string) => void;
  onPrioritized: (item: Item) => void;
  onClosed: () => void;
  onMerge: () => void;
}) {
  // As the operator's review moved it, until the hub says where it is:
  // Request changes hands the item back, Send comments keeps it.
  const [moved, setMoved] = createSignal<ItemState>();
  createEffect(
    on(
      () => props.latest.state,
      () => setMoved(undefined),
      { defer: true },
    ),
  );
  const state = () => moved() ?? props.latest.state;
  // Every comment on the item, the older revisions' too, over the diff.
  const [talking, setTalking] = createSignal(false);
  const [abandoning, setAbandoning] = createSignal(false);
  const [picking, setPicking] = createSignal(false);
  const priority = () => props.latest.priority;
  const note = () => props.latest.note;
  let toggle: HTMLButtonElement | undefined;
  let view: DiffViewControl | undefined;
  const viewed = useViewed(() => props.diff);
  // Dropped files are in the diff but not the revision's own.
  const own = createMemo(() =>
    props.diff.files.filter((file) => file.interdiff !== "dropped"),
  );
  const [showTrusted, setShowTrusted] = createSignal(false);
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
  const toReview = createMemo(() =>
    own().filter((file) => file.tier === "review"),
  );
  // The old side of an interdiff is the earlier revision replayed onto the
  // newest's base, which no revision has, so only the new side takes
  // comments, and a dropped file isn't the newest's to comment on. A file
  // shown with its own diff, unchanged or unseparated, has the real base.
  const onOwnSide = (spot: { path: string; side: string }) => {
    if (props.diff.since === null) return true;
    const file = own().find((file) => file.path === spot.path);
    return (
      file !== undefined &&
      (spot.side === "additions" ||
        file.interdiff === "unchanged" ||
        file.interdiff === "unseparated")
    );
  };
  // Every file unchanged since the revision compared against, folded or not.
  const still = createMemo(
    () =>
      new Set(
        props.diff.files
          .filter((file) => file.interdiff === "unchanged")
          .map((file) => file.path),
      ) as ReadonlySet<string>,
  );
  // Viewed files the operator was taken back into, to a comment, which stay
  // open until marked again: collapsed, they'd hide the line.
  const [opened, setOpened] = createSignal<ReadonlySet<string>>(new Set());
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
        if (viewed.paths().has(spot.path) && !opened().has(spot.path)) {
          setOpened((paths) => new Set(paths).add(spot.path));
        }
        view?.showLine(spot);
      },
      showAll: () => setTalking(true),
      hideAll: () => void hideAll(),
      onScreen: onOwnSide,
      since: () => props.diff.since,
      unchanged: still,
    },
  );
  // Since an earlier revision, files unchanged since then are left out of
  // the diff, unless they hold a thread waiting on the operator.
  const changed = createMemo(() =>
    props.diff.files.filter(
      (file) =>
        file.interdiff !== "unchanged" ||
        comments.threads.unfolded().has(file.path),
    ),
  );
  const unchanged = createMemo(() =>
    props.diff.files.filter(
      (file) =>
        file.interdiff === "unchanged" &&
        !comments.threads.unfolded().has(file.path),
    ),
  );
  // Trusted files stay out of the diff until the operator opens their group.
  const review = createMemo(() =>
    changed().filter((file) => file.tier === "review"),
  );
  const trusted = createMemo(() =>
    changed().filter((file) => file.tier === "trusted"),
  );
  const shown = createMemo(() =>
    showTrusted() ? [...review(), ...trusted()] : review(),
  );
  // A viewed mark set before the thread came back would fold it away with
  // its file, so the file opens until it's marked again, and again when the
  // screen compares against another revision.
  createEffect(
    on(
      [() => props.diff.since, comments.threads.unfolded],
      ([since, paths], before) => {
        const was = before?.[0] === since ? before[1] : undefined;
        const back = [...paths].filter((path) => !was?.has(path));
        if (back.length) setOpened((open) => new Set([...open, ...back]));
      },
    ),
  );
  // A viewed file collapses to its header, but not over the open editor,
  // which would hold the review with nothing on screen to save or cancel.
  // Only the path: the editor's state changes with every key typed, and each
  // new set walks every file's lines again.
  const writingIn = createMemo(() => comments.writing()?.path);
  const collapsed = createMemo(
    () =>
      new Set(
        [...viewed.paths()].filter(
          (path) => path !== writingIn() && !opened().has(path),
        ),
      ) as ReadonlySet<string>,
  );
  /** A mark set or cleared on an opened file ends its opening. */
  const mark = (path: string, value: boolean) => {
    if (opened().has(path)) {
      setOpened((paths) => {
        const rest = new Set(paths);
        rest.delete(path);
        return rest;
      });
    }
    viewed.mark(path, value);
  };
  const nav = useNavigation({
    files: shown,
    collapsed,
    view: () => view,
  });
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
  const writing = () => comments.editor.open() !== undefined;

  // The browser asks before a tab closes on what isn't sent: the summary and
  // reason outlive a reload in the session, but not a closed tab, and an
  // open comment isn't kept anywhere until it's saved.
  const dirty = () =>
    Boolean(props.summary.trim()) ||
    (abandoning() && Boolean(props.reason.trim())) ||
    comments.editor.holding();
  const onUnload = (event: BeforeUnloadEvent) => {
    if (dirty()) event.preventDefault();
  };
  window.addEventListener("beforeunload", onUnload);
  onCleanup(() => window.removeEventListener("beforeunload", onUnload));

  // The address says which file and line are on screen, so a reload or a
  // link pasted to another tab opens there.
  const linked = spotOf(location.hash);
  let restored = linked === undefined;
  const restore = () => {
    if (restored || !linked) return;
    restored = true;
    if (trusted().some((file) => file.path === linked.path)) {
      setShowTrusted(true);
    }
    requestAnimationFrame(() =>
      linked.line === undefined
        ? nav.showFile(linked.path)
        : nav.showSpot({ ...linked, line: linked.line }),
    );
  };
  createEffect(() => {
    const path = nav.current();
    const line = nav.cursor();
    if (!restored || path === undefined) return;
    const params = new URLSearchParams({ file: path });
    if (line && line.path === path) {
      params.set("line", String(line.line));
      if (line.side === "deletions") params.set("side", "old");
    }
    history.replaceState(
      history.state,
      "",
      `${itemHref(props.diff.item.id)}?${params}`,
    );
  });

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
    mark(file.path, value);
    if (value && file !== shown().at(-1)) nav.stepFile(1);
    else nav.showFile(file.path);
  };
  /** `c` comments on the cursor's line. */
  const comment = () => {
    const line = nav.cursor();
    if (line && reviewable(state()) && onOwnSide(line)) {
      pick({ ...line, start: line.line });
    }
  };

  /** Lines picked in the diff: for a new comment, or a draft re-placed. */
  const pick = (span: Span) => {
    if (comments.drafts.placing()) comments.drafts.place(span);
    else comments.drafts.start(span);
  };

  // An open comment takes the keys: the editor focuses a few frames after
  // `c`, and what's typed before then mustn't move the cursor away from it.
  const unlessWriting = (action: () => void) => () => {
    if (!writing()) action();
  };
  useKeys(
    {
      // Never leaves an open comment behind; what's typed for the review is
      // kept for when the operator comes back.
      Escape: () => {
        if (comments.editor.open()) comments.editor.escape();
        else if (comments.drafts.placing()) comments.drafts.stopPlacing();
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
    },
    {
      group: "Diff",
      keys: [
        ["j / k", "Next / previous line"],
        ["n / p", "Next / previous file"],
        ["] / [", "Next / previous change"],
        ["c", "Comment on the line"],
        ["v", "Mark the file viewed, then go on"],
        ["P", "Set the priority"],
        ["Esc", "Close what's open, then back to the inbox"],
      ],
    },
  );

  return (
    <>
      <div class={styles.top}>
        <ItemHeader
          item={props.diff.item}
          state={state()}
          priority={priority()}
          note={note()}
          reviewed={toReview().filter((file) => done().has(file.path)).length}
          review={toReview().length}
          trusted={own().length - toReview().length}
          failure={viewed.failure()}
        >
          <Show when={props.demo}>
            {(demo) => (
              <Demo demo={demo()} revision={props.diff.item.revision.number} />
            )}
          </Show>
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
          <Show when={props.diff.item.backend === "github" || open(state())}>
            <MoreMenu>
              {(close) => (
                <>
                  <Show when={props.diff.item.backend === "github"}>
                    <SyncGithub item={props.diff.item.id} />
                  </Show>
                  <Show when={open(state())}>
                    <Button
                      variant="danger"
                      onClick={() => {
                        close();
                        setAbandoning(true);
                      }}
                    >
                      Abandon…
                    </Button>
                  </Show>
                </>
              )}
            </MoreMenu>
          </Show>
        </ItemHeader>
        <Show when={props.failure}>
          {(failure) => (
            <div class={styles.failure}>
              <ErrorNote failure={failure()} />
              <Button onClick={() => props.onRetry()}>Retry</Button>
            </div>
          )}
        </Show>
        <Show when={abandoning() && open(state())}>
          <Abandon
            item={props.diff.item.id}
            name={props.diff.item.name}
            pullRequest={pullRequest(props.diff.item)}
            reason={props.reason}
            onReason={props.onReason}
            onCancel={() => setAbandoning(false)}
            onAbandoned={(item) => {
              setAbandoning(false);
              props.onReason("");
              setMoved(item.state);
              props.onClosed();
            }}
          />
        </Show>
        <Show when={picking()}>
          <PriorityPicker
            item={props.diff.item.id}
            name={props.diff.item.name}
            priority={priority()}
            note={note()}
            onClose={() => setPicking(false)}
            onSaved={(item) => {
              setPicking(false);
              props.onPrioritized(item);
            }}
          />
        </Show>
        <Show when={closed(props.diff.item.state)}>
          <Closed item={props.diff.item} />
        </Show>
        <Show when={props.diff.guide}>
          {(guide) => <Intent guide={guide()} />}
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
              onSwitch={() => props.onCompare("all")}
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
        onViewed={mark}
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
                : props.diff.files.some(
                      (file) => file.interdiff !== "unchanged",
                    )
                  ? `Every file changed since r${props.diff.since} is trusted.`
                  : `Nothing changed since r${props.diff.since}.`}
            </Empty>
          }
        >
          <DiffView
            files={shown()}
            collapsed={collapsed()}
            blobUrl={blobUrl}
            highlightsUrl={highlightsUrl}
            describe={describe}
            viewedBox={(file) =>
              // The newest revision doesn't change a dropped file, so it
              // has no mark to set.
              file.interdiff === "dropped"
                ? undefined
                : file.tier === "trusted"
                  ? {
                      checked: true,
                      disabledWhy: "Trusted files count as viewed",
                    }
                  : { checked: viewed.paths().has(file.path) }
            }
            onViewed={mark}
            onFile={nav.scrolledTo}
            onRow={nav.clicked}
            commentable={reviewable(state())}
            notes={comments.threads.spots()}
            note={(spot) => <Thread comments={comments} spot={spot} />}
            onComment={(span) => {
              if (reviewable(state()) && onOwnSide(span)) pick(span);
            }}
            control={(control) => {
              view = control;
              restore();
            }}
          />
        </Show>
      </div>
      <Show when={reviewable(state())}>
        <ReviewBar
          item={props.diff.item.id}
          revision={props.diff.item.revision.number}
          target={props.diff.item.target}
          state={state()}
          pullRequest={pullRequest(props.diff.item)}
          conflictsWith={props.conflictsWith}
          blocked={props.diff.blocked?.kind}
          newest={props.newest}
          unviewed={own().filter((file) => !done().has(file.path)).length}
          changed={props.diff.changed_since}
          drafts={comments.drafts.list().length}
          unplaced={comments.drafts.flagged().length}
          placing={comments.drafts.placing()?.id}
          onStopPlacing={comments.drafts.stopPlacing}
          replies={comments.replyDrafts.list().length}
          holding={comments.editor.holding()}
          writing={writing() || (abandoning() && open(state()))}
          summary={props.summary}
          onSummary={props.onSummary}
          onSubmitted={reviewed}
          onHeld={comments.held}
          onMerged={props.onMerge}
        />
      </Show>
      <DeletedDraft comments={comments} />
    </>
  );
}

/** An item on GitHub has a pull request, which merging or abandoning closes. */
function pullRequest(
  item: Item,
): { label: string; branch: string } | undefined {
  if (item.backend !== "github" || !item.link) return undefined;
  return { label: item.link.label, branch: item.branch };
}

/** The file and line a link to the item names after its `?`. */
function spotOf(
  hash: string,
):
  | { path: string; side: "additions" | "deletions"; line: number | undefined }
  | undefined {
  const query = hash.split("?")[1];
  if (!query) return undefined;
  const params = new URLSearchParams(query);
  const path = params.get("file");
  if (!path) return undefined;
  const line = Number(params.get("line"));
  return {
    path,
    side: params.get("side") === "old" ? "deletions" : "additions",
    line: Number.isInteger(line) && line > 0 ? line : undefined,
  };
}

type Draft = { summary: string; reason: string };

function draftKey(id: string): string {
  return `mergegate:draft:${id}`;
}

// The session may be unavailable, as in a private window: then drafts live
// only as long as the screen.
function readDraft(id: string): Draft {
  try {
    const kept: unknown = JSON.parse(
      sessionStorage.getItem(draftKey(id)) ?? "null",
    );
    if (kept && typeof kept === "object") {
      const { summary, reason } = kept as Partial<Draft>;
      return {
        summary: typeof summary === "string" ? summary : "",
        reason: typeof reason === "string" ? reason : "",
      };
    }
  } catch {
    // Unreadable is as good as none.
  }
  return { summary: "", reason: "" };
}

function writeDraft(id: string, draft: Draft): void {
  try {
    if (draft.summary || draft.reason) {
      sessionStorage.setItem(draftKey(id), JSON.stringify(draft));
    } else {
      sessionStorage.removeItem(draftKey(id));
    }
  } catch {
    // See `readDraft`.
  }
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
