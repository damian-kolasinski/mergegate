import { Match, Switch } from "solid-js";
import type { Blocked } from "../api/Blocked";
import type { Mergeable } from "../api/Mergeable";
import { Badge } from "../design-system";

/**
 * Whether the newest revision merges cleanly into its target, as the hub
 * last tried it. Nothing while that's unknown, nor for a conflict the
 * merge-blocked note already shows.
 */
export function MergeableBadge(props: {
  mergeable: Mergeable;
  blocked: Blocked | null;
  target: string;
}) {
  const conflict = () => {
    const mergeable = props.mergeable;
    return mergeable.kind === "conflict" && props.blocked?.kind !== "conflict"
      ? mergeable
      : undefined;
  };
  return (
    <Switch>
      <Match when={props.mergeable.kind === "clean"}>
        <Badge tone="success">Merges cleanly</Badge>
      </Match>
      <Match when={conflict()}>
        {(conflict) => (
          <Badge
            tone="warning"
            title={`Conflicts in ${conflict().paths.join(", ")}`}
          >
            Conflicts with {props.target}
          </Badge>
        )}
      </Match>
    </Switch>
  );
}
