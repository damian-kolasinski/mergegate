import { createResource, Show, untrack } from "solid-js";
import type { Inbox } from "../api/Inbox";
import { getJson } from "../app/hub";
import { useKeys } from "../app/keys";
import { go, itemHref } from "../app/router";
import { Mono } from "../design-system";
import styles from "./next.module.css";

// The items decided on since the page loaded. A comment leaves its item in
// Needs you, as likely first in line as before, so without these the way on
// bounces between the first two items and never reaches the rest.
const decided = new Set<string>();

/**
 * After a decision, the item that needs the operator next, which `Enter`
 * opens: the review goes on without a trip through the inbox.
 */
export function NextItem(props: {
  /** The item just decided on, which isn't next. */
  after: string;
  /** What the decision was, before the way on. */
  lead?: string;
}) {
  const [inbox] = createResource(() =>
    getJson<Inbox>("/api/inbox").catch(() => undefined),
  );
  decided.add(untrack(() => props.after));
  // Once every item has had a decision, the one after this in line, so
  // further rounds still go through them all.
  const next = () => {
    const waiting = inbox()?.needs_you ?? [];
    const at = waiting.findIndex((item) => item.id === props.after);
    const others = [...waiting.slice(at + 1), ...waiting.slice(0, at + 1)];
    return (
      waiting.find((item) => !decided.has(item.id)) ??
      others.find((item) => item.id !== props.after)
    );
  };
  const open = () => {
    const item = next();
    if (item) go(itemHref(item.id));
  };
  useKeys(
    { Enter: open },
    { group: "Next", keys: [["Enter", "Open the next item to review"]] },
  );
  return (
    <span class={styles.next} role="status">
      <Show when={props.lead}>
        {(lead) => <span class={styles.lead}>{lead()}</span>}
      </Show>
      <Show
        when={next()}
        fallback={
          <Show when={inbox()}>
            <span>Nothing else needs you</span>
          </Show>
        }
      >
        {(item) => (
          <a class={styles.link} href={itemHref(item().id)}>
            Next: <Mono>{item().id}</Mono> {item().title}
            <kbd>↵</kbd>
          </a>
        )}
      </Show>
    </span>
  );
}
