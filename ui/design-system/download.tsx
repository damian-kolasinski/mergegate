import type { JSX } from "solid-js";
import styles from "./button.module.css";

/**
 * A file to save, looking like an outline {@link Button}: a link, so the
 * browser downloads it as the server names it rather than leaving the page.
 */
export function Download(props: {
  href: string;
  /** What it saves; shown on hover. */
  title?: string | undefined;
  children: JSX.Element;
}) {
  return (
    <a
      class={styles.button}
      data-variant="outline"
      href={props.href}
      title={props.title}
      download=""
    >
      {props.children}
    </a>
  );
}
