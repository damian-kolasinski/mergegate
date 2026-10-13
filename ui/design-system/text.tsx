import type { JSX } from "solid-js";
import styles from "./text.module.css";

/** Secondary detail next to a primary line: ages, counts, branches. */
export function Meta(props: {
  tone?: "subtle" | "success" | "error";
  children: JSX.Element;
}) {
  return (
    <span class={styles.meta} data-tone={props.tone ?? "subtle"}>
      {props.children}
    </span>
  );
}

/** An identifier or code: ids, branches, paths. */
export function Mono(props: { children: JSX.Element }) {
  return <code class={styles.mono}>{props.children}</code>;
}
