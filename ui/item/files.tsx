import { createMemo, For, type JSX, Show } from "solid-js";
import type { File } from "../api/File";
import { rulesHref } from "../app/router";
import { CodeText, Meta, Row } from "../design-system";
import { Counts } from "./counts";
import styles from "./files.module.css";
import { describe, importance, lead } from "./why";

/**
 * The files the diff shows beside it, the current one selected: those to
 * review, then the trusted ones folded into a group of their own. Files are
 * marked viewed from their headers in the diff, which the list follows.
 * Since an earlier revision, the ones unchanged since then follow, folded
 * away: they have no header, so they're marked here. One holding a thread
 * that waits on the operator is in the diff instead, labelled unchanged.
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
  /** A mark set on a file unchanged since the earlier revision. */
  onViewed: (path: string, viewed: boolean) => void;
}) {
  // Once per list, not per entry: every entry reads it.
  const root = createMemo(() =>
    shared([...props.files, ...props.trusted, ...props.unchanged]),
  );
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
            root={root()}
            since={props.since}
            viewed={props.viewed.has(file.path)}
          />
        </Row>
      )}
    </For>
  );
  return (
    <nav class={styles.files} aria-label="Files">
      {/* Said once here, so the entries spend their width on what sets
          them apart. */}
      <Show when={root()}>
        {(folder) => (
          <p class={styles.root} title={folder()}>
            <bdi dir="ltr">{folder()}</bdi>
          </p>
        )}
      </Show>
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
                  {/* The group's summary says since when. */}
                  <FileEntry
                    file={file}
                    root={root()}
                    since={null}
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
  /** The folder every listed file is in, which the list says above them. */
  root: string;
  /** What an unchanged file is labelled unchanged since. */
  since: number | null;
  viewed: boolean;
  /** Set for a file with no header in the diff to mark it from. */
  onViewed?: (viewed: boolean) => void;
}) {
  // Counted as viewed without a mark.
  const trusted = () => props.file.tier === "trusted";
  const done = () => props.viewed || trusted();
  const slash = () => props.file.path.lastIndexOf("/");
  const name = () => props.file.path.slice(slash() + 1);
  const folder = () =>
    props.file.path.slice(0, Math.max(slash(), 0)).slice(
      // Past the root and the slash after it, which a file in the root
      // itself doesn't have.
      props.root ? props.root.length + 1 : 0,
    );
  return (
    <>
      <Show
        when={props.onViewed}
        fallback={
          <span
            class={styles.viewed}
            role={done() ? "img" : undefined}
            aria-label={done() ? "Viewed" : undefined}
            title={
              trusted()
                ? "Trusted files count as viewed"
                : props.viewed
                  ? "Viewed"
                  : undefined
            }
          />
        }
      >
        {(onViewed) => (
          <input
            type="checkbox"
            class={styles.viewed}
            checked={done()}
            disabled={trusted()}
            title={trusted() ? "Trusted files count as viewed" : undefined}
            aria-label={`Viewed ${props.file.path}`}
            onChange={(event) => {
              onViewed()(event.currentTarget.checked);
              // A focused field keeps keys from the shortcuts.
              event.currentTarget.blur();
            }}
          />
        )}
      </Show>
      {/* The list is in review order, not the tree's, so each entry says
          where it lives: the file whole, which tells the entries apart,
          then its folder under it. A deep folder gives up its start rather
          than the part next to the file. */}
      <span
        class={styles.entry}
        title={props.file.path}
        data-viewed={done() ? "" : undefined}
      >
        <span class={styles.name}>{breakable(name())}</span>
        <Show when={folder()}>
          <span class={styles.folder}>
            <bdi dir="ltr">{folder()}</bdi>
          </span>
        </Show>
        <Show when={lead(props.file)}>
          {(why) => (
            <span class={styles.why} title={describe(props.file)}>
              <CodeText text={why()} />
            </span>
          )}
        </Show>
        <span class={styles.facts}>
          <Show when={label(props.file, props.since)}>
            {(text) => <Meta>{text()}</Meta>}
          </Show>
          <Counts
            additions={props.file.additions}
            deletions={props.file.deletions}
          />
        </span>
        {/* Under the counts, so the scores line up down the list's right
            edge without taking the name's width. */}
        <Show when={importance(props.file)}>
          {(title) => (
            <span
              class={styles.score}
              role="img"
              title={title()}
              aria-label={title()}
              style={{ "--score": props.file.importance ?? 0 }}
              data-raised={props.file.raised ? "" : undefined}
            >
              {props.file.importance}
            </span>
          )}
        </Show>
      </span>
    </>
  );
}

function label(file: File, since: number | null): string | undefined {
  switch (file.interdiff) {
    case "unchanged":
      return since === null ? undefined : `unchanged since r${since}`;
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

/**
 * The folder all of `files` are in, as in `src/app`; empty when they share
 * none or there is only one, whose folder then stays on its entry.
 */
function shared(files: File[]): string {
  const [first, ...rest] = files.map((file) =>
    file.path.split("/").slice(0, -1),
  );
  if (!first || !rest.length) return "";
  let depth = first.length;
  for (const folder of rest) {
    let same = 0;
    while (same < depth && folder[same] === first[same]) same++;
    depth = same;
  }
  return first.slice(0, depth).join("/");
}

/**
 * A file name that may wrap where its words meet, as in
 * `EditProfile|View.swift`, rather than anywhere or not at all: cut short,
 * the names of a feature's files read the same.
 */
function breakable(name: string): JSX.Element {
  return (
    name
      // Not after a dot, which would leave the extension on a line of its own.
      .split(/(?<=[a-z0-9])(?=[A-Z])|(?<=[_+-])/)
      .flatMap((part, index) => (index ? [<wbr />, part] : [part]))
  );
}
