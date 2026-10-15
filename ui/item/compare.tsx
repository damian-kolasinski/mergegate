import { Show } from "solid-js";
import { Select } from "../design-system";

/**
 * What the diff is against: the merge-base, or an earlier revision the hub
 * has an interdiff from. Absent while there's none to pick.
 */
export function ComparePicker(props: {
  since: number | null;
  /** Earlier revisions, ascending. */
  interdiffs: readonly number[];
  /** A comment being written on the diff shown, which switching would lose. */
  holding: boolean;
  onChange: (since: string) => void;
}) {
  return (
    <Show when={props.interdiffs.length}>
      <Select
        label="Compare"
        disabled={props.holding}
        value={props.since === null ? "all" : String(props.since)}
        options={[
          { value: "all", label: "All changes" },
          ...[...props.interdiffs].reverse().map((revision) => ({
            value: String(revision),
            label: `Since r${revision}`,
          })),
        ]}
        onChange={props.onChange}
      />
    </Show>
  );
}
