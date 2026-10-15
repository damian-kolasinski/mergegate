import styles from "./intent.module.css";

/**
 * The agent's guide under the header: what the change is for, as it wrote
 * it. Plain text, so nothing an agent sends can run or restyle the page.
 */
export function Intent(props: { guide: string }) {
  return (
    <details class={styles.intent} open>
      <summary
        class={styles.summary}
        // A focused summary would take Enter and Space from the shortcuts.
        onClick={(event) => event.currentTarget.blur()}
      >
        Intent
      </summary>
      <div class={styles.text} role="region" aria-label="Intent">
        {props.guide.trim()}
      </div>
    </details>
  );
}
