import {
  DiffHunksRenderer,
  type DiffsHighlighter,
  type FileDiffMetadata,
  type RenderDiffFilesResult,
  type RenderDiffOptions,
  type ThemedDiffResult,
} from "@pierre/diffs";
import type { BlobHighlights } from "../api/BlobHighlights";
import type { File } from "../api/File";
import type { Span } from "../api/Span";
import type { Tag } from "../api/Tag";

/**
 * The hub's spans for a file, by the line numbers of each side. A context
 * line has the same spans on both.
 */
export type Painting = {
  deletions: Map<number, readonly Span[]>;
  additions: Map<number, readonly Span[]>;
};

/**
 * Each file's spans by path, read as CodeView renders it. There is one
 * diff view on the page, and it owns this.
 */
export const paintings = new Map<string, Painting>();

/** What the hub sent with `file`'s hunks, or nothing to paint. */
export function paintingOf(file: File): Painting | undefined {
  if (file.grammar === null || file.content.kind !== "text") return undefined;
  const painting: Painting = { deletions: new Map(), additions: new Map() };
  for (const hunk of file.content.hunks) {
    let old = hunk.old_start;
    let current = hunk.new_start;
    hunk.lines.forEach((line, index) => {
      const spans = hunk.spans[index] ?? [];
      switch (line[0]) {
        case "\\":
          return;
        case "-":
          painting.deletions.set(old++, spans);
          return;
        case "+":
          painting.additions.set(current++, spans);
          return;
        default:
          painting.deletions.set(old++, spans);
          painting.additions.set(current++, spans);
      }
    });
  }
  return painting;
}

/** Adds a whole side's spans, for the lines an expanded gap shows. */
export function paintSide(
  painting: Painting,
  side: keyof Painting,
  blob: BlobHighlights,
): void {
  blob.lines.forEach((spans, index) => painting[side].set(index + 1, spans));
}

/** Whether `a` and `b` paint the same, so the file needn't render again. */
export function samePainting(a: File, b: File): boolean {
  if (a.grammar !== b.grammar || a.missing_pack !== b.missing_pack) {
    return false;
  }
  const spans = (file: File) =>
    file.content.kind === "text"
      ? JSON.stringify(file.content.hunks.map((hunk) => hunk.spans))
      : "";
  return spans(a) === spans(b);
}

type Node = RenderDiffFilesResult["deletionLines"][number];
type Element = Extract<Node, { type: "element" }>;

type Render = (
  this: DiffHunksRenderer,
  diff: FileDiffMetadata,
  highlighter: DiffsHighlighter,
  forcePlainText?: boolean,
) => { result: ThemedDiffResult; options: RenderDiffOptions };

// CodeView highlights through this method alone, with each line's side and
// number on its node, so wrapping it paints every way a file renders:
// hunks, expanded gaps, and the plain pass before Shiki loads. It's private
// to the library; tests/highlight.spec.ts fails if an upgrade moves it.
const renderer = DiffHunksRenderer.prototype as unknown as {
  renderDiffWithHighlighter: Render;
};
const render = renderer.renderDiffWithHighlighter;
/**
 * Each diff's last render. CodeView keeps a highlighted diff's, but renders
 * a plain one's every line again whenever it renders the file at all, as
 * its window moves or a note or the viewed mark changes: 18k lines are a
 * frame's work many times over. Ours are all plain, coloured by the hub's
 * spans, and the same lines render the same. Spans come with a new diff;
 * expanding a gap fills the diff in place, with new arrays of lines and
 * hunks, so those are part of the key. One CodeView renders every diff,
 * with the same options.
 */
const kept = new WeakMap<
  FileDiffMetadata,
  Pick<FileDiffMetadata, "deletionLines" | "additionLines" | "hunks"> & {
    highlighter: DiffsHighlighter;
    plain: boolean | undefined;
    rendered: ReturnType<Render>;
  }
>();
renderer.renderDiffWithHighlighter = function (diff, highlighter, plain) {
  const last = kept.get(diff);
  if (
    last?.deletionLines === diff.deletionLines &&
    last.additionLines === diff.additionLines &&
    last.hunks === diff.hunks &&
    last.highlighter === highlighter &&
    last.plain === plain
  ) {
    return last.rendered;
  }
  const rendered = render.call(this, diff, highlighter, plain);
  const painting = paintings.get(diff.name);
  if (painting) {
    paintLines(rendered.result.code.deletionLines, painting.deletions);
    paintLines(rendered.result.code.additionLines, painting.additions);
  }
  kept.set(diff, {
    deletionLines: diff.deletionLines,
    additionLines: diff.additionLines,
    hunks: diff.hunks,
    highlighter,
    plain,
    rendered,
  });
  return rendered;
};

function paintLines(
  lines: readonly (Node | undefined)[],
  spans: Map<number, readonly Span[]>,
) {
  for (const line of lines) {
    if (line?.type !== "element") continue;
    const number = line.properties["data-line"];
    const found = typeof number === "number" ? spans.get(number) : undefined;
    if (found?.length) paintLine(line, found);
  }
}

/**
 * Splits the line's text at span edges and wraps each span's runs in an
 * element tagged with it, inside whatever already wraps the text (Shiki's
 * token, a word diff's mark), so those keep their look.
 */
function paintLine(line: Element, spans: readonly Span[]) {
  walk(line, { spans, offset: 0, next: 0 });
}

/** Where `paintLine` is in the line's text and in its spans. */
type Cursor = { spans: readonly Span[]; offset: number; next: number };

// A function of its own rather than a closure per line: a near-cap file
// paints 18k lines in the frame that opens it.
function walk(element: Element, cursor: Cursor) {
  const { spans } = cursor;
  const children: Node[] = [];
  for (const child of element.children) {
    if (child.type === "element") walk(child, cursor);
    if (child.type !== "text") {
      children.push(child);
      continue;
    }
    const text = child.value;
    const offset = cursor.offset;
    const end = offset + text.length;
    let at = offset;
    while (at < end) {
      let span = spans[cursor.next];
      while (span && spanEnd(span) <= at) span = spans[++cursor.next];
      if (!span || span[0] > at) {
        const to = Math.min(span?.[0] ?? end, end);
        children.push({
          type: "text",
          value: text.slice(at - offset, to - offset),
        });
        at = to;
        continue;
      }
      const to = Math.min(spanEnd(span), end);
      children.push({
        type: "element",
        tagName: "span",
        properties: { "data-tok": span[2] },
        children: [
          { type: "text", value: text.slice(at - offset, to - offset) },
        ],
      });
      at = to;
    }
    cursor.offset = end;
  }
  element.children = children;
}

function spanEnd(span: Span): number {
  return span[0] + span[1];
}

/**
 * Pierre's light and dark theme colours for the scopes each tag stands for,
 * so a highlighted file looks as Shiki would show it; label and literal have
 * no scope there and borrow type's and string's. A tag without one stays
 * plain.
 */
const colours: Partial<Record<Tag, [light: string, dark: string]>> = {
  attribute: ["#18a46c", "#60d199"],
  comment: ["#737373", "#737373"],
  constant: ["#1ca1c7", "#68cdf2"],
  escape: ["#16a994", "#61d5c0"],
  function: ["#693acf", "#9d6afb"],
  heading: ["#d5512f", "#ff855e"],
  keyword: ["#d32a61", "#ff678d"],
  label: ["#a631be", "#d568ea"],
  link: ["#1ca1c7", "#68cdf2"],
  literal: ["#199f43", "#5ecc71"],
  namespace: ["#d5901c", "#ffab16"],
  number: ["#1ca1c7", "#68cdf2"],
  operator: ["#636363", "#636363"],
  property: ["#d47628", "#ffa359"],
  punctuation: ["#636363", "#636363"],
  string: ["#199f43", "#5ecc71"],
  strong: ["#d5a910", "#ffd452"],
  tag: ["#d5512f", "#ff855e"],
  type: ["#a631be", "#d568ea"],
};

/** For CodeView's shadow root, where its token rule reads these. */
export const highlightCSS = [
  ...Object.entries(colours).map(
    ([tag, [light, dark]]) =>
      `[data-tok="${tag}"] { --diffs-token-light: ${light}; --diffs-token-dark: ${dark}; }`,
  ),
  `[data-tok="emphasis"] { font-style: italic; }`,
  `[data-tok="strong"], [data-tok="heading"] { font-weight: 600; }`,
  `[data-tok="link"] { text-decoration: underline; }`,
].join("\n");
