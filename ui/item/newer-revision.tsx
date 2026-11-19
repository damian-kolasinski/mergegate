import { Badge, Button, Meta } from "../design-system";
import { Note } from "./notice";

/**
 * A revision arrived while an older one is on screen. The review stays on
 * the older one until the operator switches, so nothing moves under them,
 * and switching shows all its changes; Compare narrows them to an earlier
 * revision's.
 */
export function NewerRevision(props: {
  newest: number;
  shown: number;
  /** A comment being written on r`shown`, which switching would lose. */
  holding: boolean;
  onSwitch: () => void;
}) {
  return (
    <Note label="Newer revision">
      <Badge
        tone="brand"
        title={`Comments still go to r${props.shown}; approving waits for r${props.newest}.`}
      >
        r{props.newest} submitted
      </Badge>
      <Meta>Comments still go to r{props.shown}</Meta>
      <Button variant="solid" disabled={props.holding} onClick={props.onSwitch}>
        Show r{props.newest}
      </Button>
    </Note>
  );
}
