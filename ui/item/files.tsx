import { For, Show } from "solid-js";
import type { File } from "../api/File";
import { Meta, Mono, Row } from "../design-system";
import { Counts } from "./counts";
import styles from "./files.module.css";

/**
 * The revision's files beside the diff, the current one selected: those to
 * review, then the trusted ones folded into a group of their own.
 */
export function FileList(props: {
  files: File[];
  trusted: File[];
  /** Whether the trusted group is open, and its files in the diff. */
  expanded: boolean;
  onExpand: (expanded: boolean) => void;
  viewed: ReadonlySet<string>;
  current: string | undefined;
  onShow: (path: string) => void;
  onViewed: (path: string, viewed: boolean) => void;
}) {
  const entries = (files: File[]) => (
    <For each={files}>
      {(file) => (
        <Row
          selected={file.path === props.current}
          onSelect={() => props.onShow(file.path)}
          onOpen={() => undefined}
        >
          <FileEntry
            file={file}
            viewed={props.viewed.has(file.path)}
            onViewed={(value) => props.onViewed(file.path, value)}
          />
        </Row>
      )}
    </For>
  );
  return (
    <nav class={styles.files} aria-label="Files">
      <ul>{entries(props.files)}</ul>
      <Show when={props.trusted.length}>
        <button
          type="button"
          class={styles.group}
          aria-expanded={props.expanded}
          onClick={(event) => {
            props.onExpand(!props.expanded);
            // A focused button would take Enter from the shortcuts.
            event.currentTarget.blur();
          }}
        >
          <span class={styles.chevron} aria-hidden="true">
            {props.expanded ? "▾" : "▸"}
          </span>
          Trusted ({props.trusted.length})
        </button>
        <Show when={props.expanded}>
          <ul aria-label="Trusted files">{entries(props.trusted)}</ul>
        </Show>
      </Show>
    </nav>
  );
}

function FileEntry(props: {
  file: File;
  viewed: boolean;
  onViewed: (viewed: boolean) => void;
}) {
  // Counted as viewed without a mark, so there's nothing to tick.
  const trusted = () => props.file.tier === "trusted";
  return (
    <>
      <input
        type="checkbox"
        class={styles.viewed}
        checked={props.viewed || trusted()}
        disabled={trusted()}
        aria-label={`Viewed ${props.file.path}`}
        // The row would show the file too.
        onClick={(event) => event.stopPropagation()}
        onChange={(event) => {
          props.onViewed(event.currentTarget.checked);
          // A focused field keeps keys from the shortcuts.
          event.currentTarget.blur();
        }}
      />
      <span class={styles.entry}>
        <span class={styles.path} title={props.file.path}>
          <Mono>{props.file.path}</Mono>
        </span>
        <Show when={props.file.why}>
          {(why) => (
            <span class={styles.why} title={why()}>
              <Meta>{reason(props.file, why())}</Meta>
            </span>
          )}
        </Show>
      </span>
      <Counts
        additions={props.file.additions}
        deletions={props.file.deletions}
      />
    </>
  );
}

/** Why the file is where it is, as in "trusted by R3: `*.lock` — lockfile". */
function reason(file: File, why: string): string {
  if (!file.rule) return `kept in review: ${why}`;
  return `${file.tier === "trusted" ? "trusted" : "reviewed"} by ${file.rule}: ${why}`;
}
