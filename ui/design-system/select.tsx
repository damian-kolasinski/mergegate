import { For } from "solid-js";
import styles from "./select.module.css";

/** A native drop-down, labelled for assistive tech by `label`. */
export function Select(props: {
  label: string;
  value: string;
  disabled?: boolean;
  options: readonly { value: string; label: string }[];
  onChange: (value: string) => void;
}) {
  return (
    <select
      class={styles.select}
      aria-label={props.label}
      disabled={props.disabled}
      value={props.value}
      onChange={(event) => {
        props.onChange(event.currentTarget.value);
        // A focused field keeps keys from the shortcuts.
        event.currentTarget.blur();
      }}
    >
      <For each={props.options}>
        {(option) => <option value={option.value}>{option.label}</option>}
      </For>
    </select>
  );
}
