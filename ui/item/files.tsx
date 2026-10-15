import { For, Show } from "solid-js";
import type { File } from "../api/File";
import { rulesHref } from "../app/router";
import { CodeText, Meta, Row } from "../design-system";
import { Counts } from "./counts";
import styles from "./files.module.css";
import { describe } from "./why";

/**
 * The files the diff shows beside it, the current one selected: those to
 * review, then the trusted ones folded into a group of their own. Since an
 * earlier revision, the ones unchanged since then follow, folded away, so
 * they can still be marked viewed.
 */
export function FileList(props: {
  /** Whose rules sorted the files, linked from the trusted group. */
  project: string;
  files: File[];
  trusted: File[];
  /** Whether the trusted group is open, and its files in the diff. */
  expanded: boolean;
  onExpand: (expanded: boolean) => void;
  unchanged: File[];
  since: number | null;
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
        <div class={styles.group}>
          <button
            type="button"
            class={styles.toggle}
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
          <a class={styles.rules} href={rulesHref(props.project)}>
            {props.project} rules
          </a>
        </div>
        <Show when={props.expanded}>
          <ul aria-label="Trusted files">{entries(props.trusted)}</ul>
        </Show>
      </Show>
      <Show when={props.unchanged.length}>
        <details class={styles.unchanged}>
          <summary>
            <Meta>
              {props.unchanged.length} unchanged since r{props.since}
            </Meta>
          </summary>
          <ul>
            <For each={props.unchanged}>
              {(file) => (
                <li class={styles.still}>
                  <FileEntry
                    file={file}
                    viewed={props.viewed.has(file.path)}
                    onViewed={(value) => props.onViewed(file.path, value)}
                  />
                </li>
              )}
            </For>
          </ul>
        </details>
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
  const slash = () => props.file.path.lastIndexOf("/");
  const name = () => props.file.path.slice(slash() + 1);
  const folder = () => props.file.path.slice(0, Math.max(slash(), 0));
  return (
    <>
      {/* The newest revision doesn't change a dropped file, so it has no
          mark to set. */}
      <Show
        when={props.file.interdiff !== "dropped"}
        fallback={<span class={styles.viewed} />}
      >
        <input
          type="checkbox"
          class={styles.viewed}
          checked={props.viewed || trusted()}
          disabled={trusted()}
          title={trusted() ? "Trusted files count as viewed" : undefined}
          aria-label={`Viewed ${props.file.path}`}
          // The row would show the file too.
          onClick={(event) => event.stopPropagation()}
          onChange={(event) => {
            props.onViewed(event.currentTarget.checked);
            // A focused field keeps keys from the shortcuts.
            event.currentTarget.blur();
          }}
        />
      </Show>
      <span class={styles.entry}>
        {/* The name first, where the eye lands; a deep folder gives up its
            start rather than the part next to the name. */}
        <span class={styles.path} title={props.file.path}>
          <span class={styles.name}>{name()}</span>
          <Show when={folder()}>
            <span class={styles.folder}>
              <bdi dir="ltr">{folder()}</bdi>
            </span>
          </Show>
        </span>
        <Show when={describe(props.file)}>
          {(why) => (
            <span class={styles.why} title={why()}>
              <CodeText text={why()} />
            </span>
          )}
        </Show>
      </span>
      <Show when={label(props.file)}>{(text) => <Meta>{text()}</Meta>}</Show>
      <Counts
        additions={props.file.additions}
        deletions={props.file.deletions}
      />
    </>
  );
}

function label(file: File): string | undefined {
  switch (file.interdiff) {
    case "unseparated":
      return "whole diff";
    case "dropped":
      return "dropped";
    case "added":
      return "added";
    case "deleted":
      return "deleted";
    default:
      return undefined;
  }
}
