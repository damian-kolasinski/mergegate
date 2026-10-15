import { Badge, Button, Meta } from "../design-system";
import styles from "./newer-revision.module.css";

/**
 * A revision arrived while an older one is on screen. The review stays on
 * the older one until the operator switches, so nothing moves under them.
 */
export function NewerRevision(props: {
  newest: number;
  shown: number;
  /** A comment being written on r`shown`, which switching would lose. */
  holding: boolean;
  onSwitch: () => void;
}) {
  return (
    <section class={styles.notice} aria-label="Newer revision">
      <Badge tone="brand">r{props.newest} submitted</Badge>
      <Meta>
        Comments still go to r{props.shown}; requesting changes and approving
        wait for the newest.
      </Meta>
      <span class={styles.actions}>
        <Button
          variant="solid"
          disabled={props.holding}
          onClick={props.onSwitch}
        >
          Show changes since r{props.shown}
        </Button>
      </span>
    </section>
  );
}
