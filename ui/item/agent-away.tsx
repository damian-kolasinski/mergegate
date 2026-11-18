import { Badge, Meta, Mono } from "../design-system";
import { Note } from "./notice";

/**
 * No agent reads the item's feedback and none is woken for it, so what the
 * operator sends waits until a session picks the item up. An item back with
 * an agent that read what it was sent isn't away: it's working.
 */
export function AgentAway(props: { name: string }) {
  return (
    <Note label="No agent">
      <Badge
        tone="warning"
        solid
        title="Nothing is waiting on this item, so what you send stays here until a session picks it up."
      >
        No agent
      </Badge>
      <Meta>
        What you send waits for <Mono>mg resume {props.name}</Mono>
      </Meta>
    </Note>
  );
}
