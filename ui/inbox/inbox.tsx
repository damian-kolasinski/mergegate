import { createMemo, createSignal, For, onCleanup, Show } from "solid-js";
import type { Blocked } from "../api/Blocked";
import type { ChangedSince } from "../api/ChangedSince";
import type { Inbox } from "../api/Inbox";
import type { InboxItem } from "../api/InboxItem";
import { type Failure, failureOf, getJson, onHubEvent } from "../app/hub";
import { useKeys } from "../app/keys";
import { go, itemHref, rulesHref } from "../app/router";
import { useTitle } from "../app/title";
import { PriorityBadge, PriorityPicker } from "../priority/priority";
import {
  Badge,
  Empty,
  ErrorNote,
  ExternalLink,
  Meta,
  Mono,
  Row,
  Section,
} from "../design-system";
import { AgentsPrompt } from "./agents";
import styles from "./inbox.module.css";
import { WeeklyStats } from "./weekly";

const GROUPS = [
  { key: "needs_you", title: "Needs you" },
  { key: "with_agents", title: "With agents" },
  { key: "merging", title: "Merging" },
] as const;

// Outlives the screen, so coming back from an item lands on the same row.
const [selectedId, setSelectedId] = createSignal<string>();

/** Open items in the hub's groups, refreshed whenever the hub says so. */
export function InboxScreen() {
  const [inbox, setInbox] = createSignal<Inbox>();
  const [failure, setFailure] = createSignal<Failure>();
  let latest = 0;
  const load = async () => {
    // Responses can arrive out of order; only the newest request's counts.
    const request = ++latest;
    try {
      const next = await getJson<Inbox>("/api/inbox");
      if (request !== latest) return;
      setInbox(next);
      setFailure(undefined);
    } catch (error) {
      if (request !== latest) return;
      setFailure(failureOf(error));
    }
  };
  void load();
  onHubEvent("inbox.changed", () => void load());
  // The tab says how many items wait on the operator, from any other tab.
  useTitle(() => {
    const waiting = inbox()?.needs_you.length;
    return waiting ? `(${waiting}) Inbox` : "Inbox";
  });

  const columns = createMemo(() => {
    const current = inbox();
    return current ? columnsOf(current) : { flags: false, link: false };
  });

  const [now, setNow] = createSignal(Date.now());
  const clock = setInterval(() => {
    setNow(Date.now());
    // Calendar sections roll over even when no item changes overnight.
    void load();
  }, 30_000);
  onCleanup(() => clearInterval(clock));

  // `j/k` walk the open groups as one list, in the order they're shown;
  // what closed today is there to look back on, not to work through.
  const items = createMemo(() => {
    const current = inbox();
    return current ? GROUPS.flatMap((group) => current[group.key]) : [];
  });
  // An item that leaves the list, say once it's merged, hands the selection
  // to the first one waiting on the operator.
  const selected = createMemo(() => {
    const list = items();
    return list.find((item) => item.id === selectedId())?.id ?? list[0]?.id;
  });
  const move = (step: number) => {
    const list = items();
    const index = list.findIndex((item) => item.id === selected());
    const next = list[Math.min(Math.max(index + step, 0), list.length - 1)];
    if (next) setSelectedId(next.id);
  };
  const open = (id = selected()) => {
    if (id !== undefined) go(itemHref(id));
  };
  // The item `p` opened the priority picker on.
  const [picking, setPicking] = createSignal<InboxItem>();
  useKeys(
    {
      j: () => move(1),
      k: () => move(-1),
      ArrowDown: () => move(1),
      ArrowUp: () => move(-1),
      Enter: () => open(),
      p: () => setPicking(items().find((item) => item.id === selected())),
    },
    {
      group: "Inbox",
      keys: [
        ["j / k", "Next / previous item"],
        ["Enter", "Open the item"],
        ["p", "Set the item's priority"],
      ],
    },
  );

  // A failed refresh keeps the last inbox on screen, under the message.
  return (
    <div class={styles.inbox}>
      <AgentsPrompt />
      <Show when={failure()}>
        {(current) => (
          <Show
            when={inbox()}
            fallback={
              <Empty>
                <ErrorNote failure={current()} />
              </Empty>
            }
          >
            <div class={styles.failure}>
              <ErrorNote failure={current()} />
            </div>
          </Show>
        )}
      </Show>
      <Show when={inbox()}>
        {(current) => (
          <>
            <Show when={current().rules.length}>
              <ul class={styles.rules} aria-label="Rule changes">
                <For each={current().rules}>
                  {(pending) => (
                    <li>
                      <a href={rulesHref(pending.project)}>
                        <Badge tone="warning">
                          {pending.project} · {pending.changes}{" "}
                          {pending.changes === 1
                            ? "rule change to approve"
                            : "rule changes to approve"}
                        </Badge>
                      </a>
                    </li>
                  )}
                </For>
              </ul>
            </Show>
            <For each={GROUPS}>
              {(group) => (
                <Show
                  when={
                    group.key === "needs_you" || current()[group.key].length
                  }
                >
                  <Section
                    title={group.title}
                    count={current()[group.key].length}
                  >
                    <Show
                      when={current()[group.key].length}
                      fallback={
                        <Empty>
                          <Show
                            when={
                              current().with_agents.length +
                              current().merging.length
                            }
                            fallback={
                              <>
                                Nothing to review. Items show up here when an
                                agent runs <code>mg submit</code>.
                              </>
                            }
                          >
                            {(working) => (
                              <>
                                All caught up — agents are working on{" "}
                                {working()} {working() === 1 ? "item" : "items"}
                                .
                              </>
                            )}
                          </Show>
                        </Empty>
                      }
                    >
                      <ul>
                        <For each={current()[group.key]}>
                          {(item) => (
                            <Entry
                              item={item}
                              columns={columns()}
                              now={now()}
                              selected={item.id === selected()}
                              onSelect={() => setSelectedId(item.id)}
                              onOpen={() => open(item.id)}
                            />
                          )}
                        </For>
                      </ul>
                    </Show>
                  </Section>
                </Show>
              )}
            </For>
            <Show when={current().closed.length}>
              <details class={styles.closed}>
                <summary
                  // A focused summary would take Enter from the shortcuts.
                  onClick={(event) => event.currentTarget.blur()}
                >
                  Closed today ({current().closed.length})
                </summary>
                <ul>
                  <For each={current().closed}>
                    {(item) => (
                      <Entry
                        item={item}
                        columns={columns()}
                        now={now()}
                        selected={false}
                        onSelect={() => undefined}
                        onOpen={() => open(item.id)}
                      />
                    )}
                  </For>
                </ul>
              </details>
            </Show>
            <Show when={current().weekly.monday}>
              <WeeklyStats week={current().weekly} />
            </Show>
          </>
        )}
      </Show>
      <Show when={picking()} keyed>
        {(item) => (
          <PriorityPicker
            item={item.id}
            name={item.name}
            priority={item.priority}
            note={item.note}
            onClose={() => setPicking(undefined)}
            onSaved={() => {
              setPicking(undefined);
              void load();
            }}
          />
        )}
      </Show>
    </div>
  );
}

/** The optional columns: each is there when some row has something in it. */
type Columns = { flags: boolean; link: boolean };

function columnsOf(inbox: Inbox): Columns {
  const all = [...GROUPS.flatMap((group) => inbox[group.key]), ...inbox.closed];
  return {
    flags: all.some(flagged),
    link: all.some((item) => item.link),
  };
}

/** Whether the row shows any of the flags `Entry` draws. */
function flagged(item: InboxItem): boolean {
  return Boolean(
    item.questions ||
      item.rejections ||
      item.votes ||
      item.state === "changes_requested" ||
      item.changed_since ||
      item.parent ||
      item.blocked ||
      unblockedConflict(item) ||
      item.demo,
  );
}

function Entry(props: {
  item: InboxItem;
  columns: Columns;
  now: number;
  selected: boolean;
  onSelect: () => void;
  onOpen: () => void;
}) {
  return (
    <Row
      selected={props.selected}
      onSelect={props.onSelect}
      onOpen={props.onOpen}
    >
      <span class={styles.id} title={props.item.id}>
        <Mono>{props.item.name}</Mono>
      </span>
      <span class={styles.priority}>
        <PriorityBadge
          priority={props.item.priority}
          note={props.item.note}
          keyHint="p"
        />
      </span>
      {/* A real link, so a middle or ⌘ click opens the item in a new tab. */}
      <a
        class={styles.title}
        href={itemHref(props.item.id)}
        title={props.item.branch}
      >
        {props.item.title}
        <Show when={props.item.note}>
          {(note) => (
            <span class={styles.note} title={note()}>
              {" · "}
              {note()}
            </span>
          )}
        </Show>
      </a>
      {/* One column for every flag, so however many a row has, the cells
          after it stay in line with the rows around it. */}
      <Show when={props.columns.flags}>
        <span class={styles.flags}>
          <Show when={props.item.questions}>
            {(count) => (
              <span
                class={styles.flag}
                title="The agent asked: it waits on you"
              >
                <Badge tone="brand" solid>
                  {count()} {count() === 1 ? "question" : "questions"}
                </Badge>
              </span>
            )}
          </Show>
          <Show when={props.item.rejections}>
            {(count) => (
              <span
                class={styles.flag}
                title="The agent rejected a comment: accept it or override"
              >
                <Badge tone="warning" solid>
                  {count()} {count() === 1 ? "rejection" : "rejections"}
                </Badge>
              </span>
            )}
          </Show>
          <Show when={props.item.votes}>
            {(count) => (
              <span
                class={styles.flag}
                title="Comments from GitHub wait for your vote before the agent gets them"
              >
                <Badge tone="warning" solid>
                  {count()} to vote on
                </Badge>
              </span>
            )}
          </Show>
          <Show when={props.item.state === "changes_requested"}>
            <span
              class={styles.flag}
              title={`Changes requested: waiting for r${props.item.revision + 1}`}
            >
              <Meta>
                changes requested · waiting for r{props.item.revision + 1}
              </Meta>
            </span>
          </Show>
          <Show when={props.item.changed_since}>
            {(changed) => (
              <span class={styles.flag}>
                <Meta>
                  {changedCount(changed())}changed since{" "}
                  {changed().approved ? "approved " : ""}r{changed().revision}
                </Meta>
              </span>
            )}
          </Show>
          <Show when={props.item.parent}>
            {(parent) => (
              <span
                class={styles.flag}
                title={`Stacked on ${parent()}: reviewed as its own diff, it lands right after`}
              >
                <Badge tone={props.item.behind ? "warning" : "neutral"}>
                  ↳ {props.item.behind ? "behind" : "on"} {parent()}
                </Badge>
              </span>
            )}
          </Show>
          <Show when={props.item.blocked}>
            {(blocked) => (
              <span class={styles.flag} title={why(blocked())}>
                <Badge
                  tone={blocked().kind === "waiting" ? "brand" : "warning"}
                  solid
                >
                  {blockedLabel(blocked(), props.item.target)}
                </Badge>
              </span>
            )}
          </Show>
          <Show when={unblockedConflict(props.item)}>
            {(conflict) => (
              <span
                class={styles.flag}
                title={`Conflicts in ${conflict().paths.join(", ")}`}
              >
                <Badge tone="warning" solid>
                  Conflicts with {props.item.target}
                </Badge>
              </span>
            )}
          </Show>
          <Show when={props.item.demo}>
            <span class={styles.flag} title="Has screenshots or recordings">
              <Badge tone="brand">▶ demo</Badge>
            </span>
          </Show>
        </span>
      </Show>
      <Show when={props.columns.link}>
        <span class={styles.link}>
          <Show when={props.item.link}>
            {(link) => (
              <ExternalLink href={link().url} title={link().title}>
                View on GitHub
              </ExternalLink>
            )}
          </Show>
        </span>
      </Show>
      {/* A zero count leaves its cell empty so the columns stay aligned. */}
      <span class={styles.figure}>
        <Show when={props.item.additions}>
          <Meta tone="added">+{props.item.additions}</Meta>
        </Show>
      </span>
      <span class={styles.figure}>
        <Show when={props.item.deletions}>
          <Meta tone="removed">−{props.item.deletions}</Meta>
        </Show>
      </span>
      <span class={styles.files} title={trusted(props.item)}>
        <Meta>{files(props.item)}</Meta>
      </span>
      <span
        class={styles.age}
        title={new Date(props.item.updated_at).toLocaleString()}
      >
        <Meta>{age(props.item.updated_at, props.now)}</Meta>
      </span>
    </Row>
  );
}

/** `4 of 6 files` when rules trusted some, so the total sets them apart. */
function files(item: InboxItem): string {
  const all = `${item.files} ${item.files === 1 ? "file" : "files"}`;
  if (item.review_files === item.files) return all;
  return `${item.review_files} of ${all}`;
}

function trusted(item: InboxItem): string | undefined {
  const count = item.files - item.review_files;
  if (!count) return undefined;
  return `${item.review_files} to review, ${count} trusted by the rules`;
}

/**
 * `3 ` of `3 changed since`, or nothing for an approval that stands yet
 * didn't carry: something changed, if only a trusted file or one the rebase
 * conflicted on.
 */
function changedCount(changed: ChangedSince): string {
  return changed.approved && !changed.files ? "" : `${changed.files} `;
}

/** A conflict with the target that no merge-blocked flag already says. */
function unblockedConflict(item: InboxItem) {
  const mergeable = item.mergeable;
  return mergeable.kind === "conflict" && item.blocked?.kind !== "conflict"
    ? mergeable
    : undefined;
}

function blockedLabel(blocked: Blocked, target: string): string {
  switch (blocked.kind) {
    case "held":
      return "Merge held";
    case "conflict":
      return `Conflicts with ${target}`;
    case "waiting":
      return "Approved";
    case "behind":
      return "Behind";
  }
}

function why(blocked: Blocked): string {
  switch (blocked.kind) {
    case "held":
      return blocked.reason.split("\n\n")[0] ?? "";
    case "conflict":
      return `Conflicts in ${blocked.paths.join(", ")}`;
    case "waiting":
      return `Merges right after ${blocked.parent} lands`;
    case "behind":
      return `Its diff didn't apply once ${blocked.parent} merged: waiting for its agent to rebase`;
  }
}

/** How long ago `timestamp` was, as short as a column allows: `5m`, `2h`. */
function age(timestamp: string, now: number): string {
  const minutes = Math.floor((now - Date.parse(timestamp)) / 60_000);
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  return `${Math.floor(hours / 24)}d`;
}
