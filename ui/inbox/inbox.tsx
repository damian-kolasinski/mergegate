import { createMemo, createSignal, For, onCleanup, Show } from "solid-js";
import type { Blocked } from "../api/Blocked";
import type { Inbox } from "../api/Inbox";
import type { InboxItem } from "../api/InboxItem";
import { getJson, onHubEvent } from "../app/hub";
import { useKeys } from "../app/keys";
import { itemHref } from "../app/router";
import {
  Badge,
  Empty,
  ExternalLink,
  Meta,
  Mono,
  Row,
  Section,
} from "../design-system";
import styles from "./inbox.module.css";

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
  const [failure, setFailure] = createSignal<string>();
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
      setFailure(error instanceof Error ? error.message : String(error));
    }
  };
  void load();
  onHubEvent("inbox.changed", () => void load());

  const [now, setNow] = createSignal(Date.now());
  const clock = setInterval(() => setNow(Date.now()), 30_000);
  onCleanup(() => clearInterval(clock));

  // `j/k` walk all groups as one list, in the order they're shown.
  const items = createMemo(() => {
    const current = inbox();
    return current ? GROUPS.flatMap((group) => current[group.key]) : [];
  });
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
    if (id !== undefined) location.hash = itemHref(id);
  };
  useKeys({
    j: () => move(1),
    k: () => move(-1),
    ArrowDown: () => move(1),
    ArrowUp: () => move(-1),
    Enter: () => open(),
  });

  // A failed refresh keeps the last inbox on screen, under the message.
  return (
    <>
      <Show when={failure()}>{(message) => <Empty>{message()}</Empty>}</Show>
      <Show when={inbox()}>
        {(current) => (
          <For each={GROUPS}>
            {(group) => (
              <Show
                when={group.key === "needs_you" || current()[group.key].length}
              >
                <Section
                  title={group.title}
                  count={current()[group.key].length}
                >
                  <Show
                    when={current()[group.key].length}
                    fallback={
                      <Empty>
                        Nothing to review. Items show up here when an agent runs{" "}
                        <code>mg submit</code>.
                      </Empty>
                    }
                  >
                    <ul>
                      <For each={current()[group.key]}>
                        {(item) => (
                          <Entry
                            item={item}
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
        )}
      </Show>
    </>
  );
}

function Entry(props: {
  item: InboxItem;
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
      <span class={styles.id}>
        <Mono>{props.item.id}</Mono>
      </span>
      <span class={styles.title} title={props.item.branch}>
        {props.item.title}
      </span>
      <Show when={props.item.pull}>
        {(pull) => (
          <span class={styles.flag}>
            <ExternalLink href={pull().url} title="Pull request on GitHub">
              #{pull().number}
            </ExternalLink>
          </span>
        )}
      </Show>
      <Show when={props.item.blocked}>
        {(blocked) => (
          <span class={styles.flag} title={why(blocked())}>
            <Badge tone="warning">
              {blocked().kind === "held" ? "Merge held" : "Conflict"}
            </Badge>
          </span>
        )}
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
      <span class={styles.figure}>
        <Meta>
          {props.item.files} {props.item.files === 1 ? "file" : "files"}
        </Meta>
      </span>
      <span class={styles.figure} title={props.item.updated_at}>
        <Meta>{age(props.item.updated_at, props.now)}</Meta>
      </span>
    </Row>
  );
}

function why(blocked: Blocked): string {
  if (blocked.kind === "held") return blocked.reason;
  return `Conflicts in ${blocked.paths.join(", ")}`;
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
