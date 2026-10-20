import {
  createEffect,
  createMemo,
  createSignal,
  For,
  type JSX,
  Show,
} from "solid-js";
import { Portal } from "solid-js/web";
import type { File } from "../api/File";
import { useKeys } from "../app/keys";
import { rulesHref } from "../app/router";
import { Meta, Row } from "../design-system";
import { Counts } from "./counts";
import styles from "./files.module.css";
import { Note, noted } from "./note";
import { type Group, type Leaf, type Node, tree } from "./tree";
import { describe, importance } from "./why";

/**
 * The files the diff shows beside it, the current one selected, laid out
 * by where they live: each module, its folders, then its files, so a file
 * reads as part of what it changes. Those to review come first, then the
 * trusted ones folded into a group of their own. Why a file is where it is
 * waits behind its ⓘ, or `i` for the current one. Files are marked viewed
 * from their headers in the diff, which the list follows. Since an earlier
 * revision, the ones unchanged since then follow, folded away: they have
 * no header, so they're marked here. One holding a thread that waits on
 * the operator is in the diff instead, labelled unchanged.
 */
export function FileList(props: {
  /** Whose rules sorted the files, linked from the trusted group. */
  project: string;
  /** In the tree's order, as the diff shows them. */
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
  /** Whether a comment is open, which takes the keys before its editor
   * has focus. */
  writing: boolean;
}) {
  const review = createMemo(() => tree(props.files, "r:"));
  const trusted = createMemo(() => tree(props.trusted, "t:"));
  // By key, so a group stays folded as the files reload.
  const [folded, setFolded] = createSignal<ReadonlySet<string>>(new Set());
  const [noting, setNoting] = createSignal(false);
  // The cursor never lands in a folded group.
  createEffect(() => {
    const current = props.current;
    if (current === undefined) return;
    const holding = [...review(), ...trusted()]
      .flatMap(groups)
      .filter((group) => group.paths.has(current))
      .map((group) => group.key);
    setFolded((keys) =>
      holding.some((key) => keys.has(key))
        ? new Set([...keys].filter((key) => !holding.includes(key)))
        : keys,
    );
  });
  const fold = (key: string) =>
    setFolded((keys) => {
      const next = new Set(keys);
      if (!next.delete(key)) next.add(key);
      return next;
    });
  const currentFile = () =>
    [...props.files, ...props.trusted].find(
      (file) => file.path === props.current,
    );
  useKeys(
    {
      i: () => {
        if (props.writing) return;
        const file = currentFile();
        setNoting(!noting() && file !== undefined && noted(file));
      },
    },
    { group: "Files", keys: [["i", "Why the current file is here"]] },
  );

  const entry = (file: File, depth: number, leaf: Leaf | undefined) => (
    <Row
      selected={file.path === props.current}
      onSelect={() => props.onShow(file.path)}
      onOpen={() => undefined}
    >
      <FileEntry
        file={file}
        depth={depth}
        tag={leaf?.tag ?? null}
        test={!leaf}
        since={props.since}
        viewed={props.viewed.has(file.path)}
        // Shown only for a file with something to say.
        noting={noting() && file.path === props.current && noted(file)}
        // Before the row selects the file, which it then notes.
        onNote={() => setNoting(!(noting() && file.path === props.current))}
        onClose={() => setNoting(false)}
      />
    </Row>
  );
  // Files run in lists between their groups, so the list items are the
  // files alone, in the tree's order.
  const branch = (nodes: Node[], depth: number): JSX.Element => (
    <For each={runs(nodes)}>
      {(run) => (
        <Show
          when={"leaves" in run ? run.leaves : undefined}
          fallback={
            <Show when={"group" in run ? run.group : undefined}>
              {(group) => (
                <GroupRow
                  group={group()}
                  depth={depth}
                  open={!folded().has(group().key)}
                  onFold={() => fold(group().key)}
                >
                  {branch(group().nodes, depth + 1)}
                </GroupRow>
              )}
            </Show>
          }
        >
          {(leaves) => (
            <ul>
              <For each={leaves()}>
                {(leaf) => (
                  <>
                    {entry(leaf.file, depth, leaf)}
                    <For each={leaf.tests}>
                      {(test) => entry(test, depth, undefined)}
                    </For>
                  </>
                )}
              </For>
            </ul>
          )}
        </Show>
      )}
    </For>
  );
  return (
    <nav class={styles.files} aria-label="Files">
      {branch(review(), 0)}
      <Show when={props.trusted.length}>
        <div class={styles.trusted}>
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
            <span class={styles.chevron} aria-hidden="true" />
            Trusted ({props.trusted.length})
          </button>
          <a class={styles.rules} href={rulesHref(props.project)}>
            {props.project} rules
          </a>
        </div>
        <Show when={props.expanded}>
          <div role="group" aria-label="Trusted files">
            {branch(trusted(), 0)}
          </div>
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
                  {/* The group's summary says since when, and not being in
                      the diff, it's out of the tree. */}
                  <FileEntry
                    file={file}
                    depth={0}
                    tag={null}
                    test={false}
                    since={null}
                    viewed={props.viewed.has(file.path)}
                    folder
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

/** `nodes` with each run of files between groups together. */
function runs(nodes: Node[]): ({ leaves: Leaf[] } | { group: Group })[] {
  const out: ({ leaves: Leaf[] } | { group: Group })[] = [];
  for (const node of nodes) {
    const last = out.at(-1);
    if (node.kind !== "file") out.push({ group: node });
    else if (last && "leaves" in last) last.leaves.push(node);
    else out.push({ leaves: [node] });
  }
  return out;
}

function groups(node: Node): Group[] {
  return node.kind === "file" ? [] : [node, ...node.nodes.flatMap(groups)];
}

/** A module or folder, which folds away what's in it. */
function GroupRow(props: {
  group: Group;
  depth: number;
  open: boolean;
  onFold: () => void;
  children: JSX.Element;
}) {
  return (
    <div
      role="group"
      aria-label={props.group.name}
      class={styles.group}
      data-kind={props.group.kind}
    >
      <button
        type="button"
        class={styles.head}
        style={{ "--depth": props.depth }}
        aria-expanded={props.open}
        onClick={(event) => {
          props.onFold();
          // A focused button would take Enter from the shortcuts.
          event.currentTarget.blur();
        }}
      >
        <span class={styles.chevron} aria-hidden="true" />
        <span class={styles.label}>
          <span class={styles.title} title={props.group.name}>
            <bdi dir="ltr">
              {props.group.name}
              {props.group.kind === "folder" ? "/" : ""}
            </bdi>
          </span>
          <Show when={props.group.sub}>
            {(sub) => (
              <span class={styles.sub} title={sub()}>
                <bdi dir="ltr">{sub()}</bdi>
              </span>
            )}
          </Show>
        </span>
        <Counts
          additions={props.group.additions}
          deletions={props.group.deletions}
        />
        {/* The most important file in it, so a folded group still says
            whether it needs reading. */}
        <Show when={props.group.peak !== null && props.group.peak} keyed>
          {(peak) => (
            <span
              class={styles.peak}
              data-band={band(peak)}
              role="img"
              aria-label={`Peaks at ${peak}/100`}
              title={`Its most important file is judged ${peak}/100`}
            >
              {peak}
            </span>
          )}
        </Show>
      </button>
      <Show when={props.open}>{props.children}</Show>
    </div>
  );
}

function FileEntry(props: {
  file: File;
  depth: number;
  /** What else than plain code it is, as `Tests`. */
  tag: string | null;
  /** A test, under the file it covers. */
  test: boolean;
  /** What an unchanged file is labelled unchanged since. */
  since: number | null;
  viewed: boolean;
  /** Whether its note is open. */
  noting?: boolean;
  onNote?: () => void;
  onClose?: () => void;
  /** Says its folder, for a file listed outside the tree. */
  folder?: boolean;
  /** Set for a file with no header in the diff to mark it from. */
  onViewed?: (viewed: boolean) => void;
}) {
  let row: HTMLSpanElement | undefined;
  // Counted as viewed without a mark.
  const trusted = () => props.file.tier === "trusted";
  const done = () => props.viewed || trusted();
  const slash = () => props.file.path.lastIndexOf("/");
  const name = () => props.file.path.slice(slash() + 1);
  return (
    <span
      ref={row}
      class={styles.entry}
      style={{ "--depth": props.depth }}
      title={props.file.path}
      data-viewed={done() ? "" : undefined}
      data-test={props.test ? "" : undefined}
    >
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
      <span class={styles.name}>
        {named(name())}
        <Show when={props.tag}>
          {(tag) => <span class={styles.tag}>{tag()}</span>}
        </Show>
        <Show when={props.folder && slash() > 0}>
          <span class={styles.folder}>
            <bdi dir="ltr">{props.file.path.slice(0, slash())}</bdi>
          </span>
        </Show>
      </span>
      <span class={styles.facts}>
        <Show when={label(props.file, props.since)}>
          {(text) => <Meta>{text()}</Meta>}
        </Show>
        <Counts
          additions={props.file.additions}
          deletions={props.file.deletions}
        />
      </span>
      <Show when={importance(props.file)}>
        {(title) => (
          <span
            class={styles.score}
            role="img"
            title={title()}
            aria-label={title()}
            data-band={band(props.file.importance ?? 0)}
            data-raised={props.file.raised ? "" : undefined}
          >
            {props.file.importance}
          </span>
        )}
      </Show>
      <Show when={props.onNote && noted(props.file)}>
        <button
          type="button"
          class={styles.info}
          data-note
          aria-label={`Why ${props.file.path} is here`}
          aria-expanded={props.noting ?? false}
          // In one line on hover; the note has it in parts.
          title={describe(props.file)}
          onClick={(event) => {
            props.onNote?.();
            event.currentTarget.blur();
          }}
        />
      </Show>
      {/* Out of the row, so it's no part of the list item nor under the
          row's title. */}
      <Show when={props.noting && row}>
        {(anchor) => (
          <Portal>
            <Note
              file={props.file}
              anchor={anchor()}
              onClose={() => props.onClose?.()}
            />
          </Portal>
        )}
      </Show>
    </span>
  );
}

/** A judge score's band, which its chip's weight follows. */
function band(score: number): "high" | "mid" | "low" {
  return score >= 70 ? "high" : score >= 40 ? "mid" : "low";
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
 * A file name that may wrap where its words meet, as in
 * `EditProfile|View.swift`, rather than anywhere or not at all: cut short,
 * the names of a feature's files read the same. Its extension steps back,
 * as the stem is what tells them apart.
 */
function named(name: string): JSX.Element {
  const dot = name.lastIndexOf(".");
  const stem = dot > 0 ? name.slice(0, dot) : name;
  return (
    <>
      {stem
        .split(/(?<=[a-z0-9])(?=[A-Z])|(?<=[_+-])/)
        .flatMap((part, index) => (index ? [<wbr />, part] : [part]))}
      <Show when={dot > 0}>
        <span class={styles.ext}>{name.slice(dot)}</span>
      </Show>
    </>
  );
}
