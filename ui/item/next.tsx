import { createResource, Show } from "solid-js";
import type { Inbox } from "../api/Inbox";
import { getJson } from "../app/hub";
import { useKeys } from "../app/keys";
import { go, itemHref } from "../app/router";
import { Mono } from "../design-system";
import styles from "./next.module.css";

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
  const next = () => inbox()?.needs_you.find((item) => item.id !== props.after);
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
