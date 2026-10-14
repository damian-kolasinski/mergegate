import type { JSX } from "solid-js";
import styles from "./external-link.module.css";

/**
 * A link out of mergegate, such as an item's pull request, opened in a new
 * tab. A click on it stays off any row it sits in, which would open the row.
 */
export function ExternalLink(props: {
  href: string;
  title?: string;
  children: JSX.Element;
}) {
  return (
    <a
      class={styles.link}
      href={props.href}
      title={props.title}
      target="_blank"
      rel="noopener noreferrer"
      onClick={(event) => event.stopPropagation()}
    >
      {props.children}
      <span aria-hidden="true"> ↗</span>
    </a>
  );
}
