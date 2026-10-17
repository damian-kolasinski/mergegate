import { Badge, Meta, Mono } from "../design-system";
import styles from "./agent-away.module.css";

/**
 * No agent reads the item's feedback and none is woken for it, so what the
 * operator sends waits until a session picks the item up. An item back with
 * an agent that read what it was sent isn't away: it's working.
 */
export function AgentAway(props: { name: string }) {
  return (
    <section class={styles.notice} aria-label="No agent">
      <Badge tone="warning" solid>
        No agent
      </Badge>
      <Meta>
        Nothing is waiting on this item, so what you send stays here until a
        session picks it up with <Mono>mg resume {props.name}</Mono>.
      </Meta>
    </section>
  );
}
