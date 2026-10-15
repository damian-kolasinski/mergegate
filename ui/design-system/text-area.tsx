import { isSubmitKey } from "../app/keys";
import styles from "./text-area.module.css";

/**
 * Multi-line text the operator writes: a summary, a comment. `Escape` gives
 * the keys back to the page's shortcuts; `⌘↩` sends it when `onSubmit` is set.
 */
export function TextArea(props: {
  label: string;
  value: string;
  placeholder?: string;
  rows?: number;
  disabled?: boolean;
  onInput: (value: string) => void;
  onSubmit?: () => void;
  ref?: (element: HTMLTextAreaElement) => void;
}) {
  return (
    <textarea
      ref={(element) => props.ref?.(element)}
      class={styles.textArea}
      aria-label={props.label}
      placeholder={props.placeholder}
      rows={props.rows ?? 2}
      disabled={props.disabled}
      value={props.value}
      onInput={(event) => props.onInput(event.currentTarget.value)}
      onKeyDown={(event) => {
        if (event.key === "Escape") event.currentTarget.blur();
        else if (props.onSubmit && isSubmitKey(event)) {
          event.preventDefault();
          props.onSubmit();
        }
      }}
    />
  );
}
