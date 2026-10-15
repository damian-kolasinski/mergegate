// The part of Markdown a README or a design note uses, parsed into a tree the
// preview renders as elements. Nothing here becomes HTML: raw HTML stays
// text, and an image shows as its alt text, so a preview loads nothing.

export type Block =
  | { kind: "heading"; level: number; text: Inline[] }
  | { kind: "paragraph"; text: Inline[] }
  | { kind: "code"; lang: string; text: string }
  | { kind: "quote"; blocks: Block[] }
  | { kind: "list"; ordered: boolean; start: number; items: Block[][] }
  | { kind: "table"; head: Inline[][]; rows: Inline[][][] }
  | { kind: "rule" };

export type Inline =
  | { kind: "text"; text: string }
  | { kind: "code"; text: string }
  | { kind: "strong" | "em" | "del"; text: Inline[] }
  | { kind: "link"; href: string; text: Inline[] }
  | { kind: "image"; alt: string }
  | { kind: "break" };

const FENCE = /^ {0,3}(`{3,}|~{3,})\s*([^`\s]*)/;
const HEADING = /^ {0,3}(#{1,6})(?:\s+(.*?))?(?:\s+#+)?\s*$/;
const RULE = /^ {0,3}([-*_])(?:\s*\1){2,}\s*$/;
const QUOTE = /^ {0,3}> ?(.*)$/;
const ITEM = /^( {0,3})([-*+]|(\d{1,9})[.)])(\s+|$)(.*)$/;
const TABLE_SPLIT = /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/;

export function parseMarkdown(source: string): Block[] {
  return blocks(source.replace(/\r\n?/g, "\n").split("\n"));
}

function blocks(lines: string[]): Block[] {
  const out: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i] ?? "";
    if (line.trim() === "") {
      i++;
      continue;
    }
    const fence = FENCE.exec(line);
    if (fence) {
      const mark = fence[1] ?? "```";
      const body: string[] = [];
      i++;
      while (i < lines.length && !closes(lines[i] ?? "", mark)) {
        body.push(lines[i] ?? "");
        i++;
      }
      i++;
      out.push({ kind: "code", lang: fence[2] ?? "", text: body.join("\n") });
      continue;
    }
    const heading = HEADING.exec(line);
    if (heading) {
      out.push({
        kind: "heading",
        level: heading[1]?.length ?? 1,
        text: inlines(heading[2] ?? ""),
      });
      i++;
      continue;
    }
    if (RULE.test(line)) {
      out.push({ kind: "rule" });
      i++;
      continue;
    }
    if (QUOTE.test(line)) {
      const body: string[] = [];
      while (i < lines.length && (lines[i] ?? "").trim() !== "") {
        const quoted = QUOTE.exec(lines[i] ?? "");
        body.push(quoted ? (quoted[1] ?? "") : (lines[i] ?? ""));
        i++;
      }
      out.push({ kind: "quote", blocks: blocks(body) });
      continue;
    }
    if (ITEM.test(line)) {
      i = list(lines, i, out);
      continue;
    }
    if (line.includes("|") && TABLE_SPLIT.test(lines[i + 1] ?? "")) {
      const head = cells(line);
      const rows: Inline[][][] = [];
      i += 2;
      while (i < lines.length && (lines[i] ?? "").includes("|")) {
        rows.push(cells(lines[i] ?? ""));
        i++;
      }
      out.push({ kind: "table", head, rows });
      continue;
    }
    const text: string[] = [];
    while (i < lines.length && !interrupts(lines[i] ?? "")) {
      text.push(lines[i] ?? "");
      i++;
    }
    out.push({ kind: "paragraph", text: inlines(text.join("\n")) });
  }
  return out;
}

/**
 * Reads the list starting at `lines[start]` into `out` and returns the line
 * after it. An item holds the lines indented past its marker, so a nested
 * list is parsed as blocks of its item.
 */
function list(lines: string[], start: number, out: Block[]): number {
  const first = ITEM.exec(lines[start] ?? "");
  const ordered = first?.[3] !== undefined;
  const items: Block[][] = [];
  let i = start;
  while (i < lines.length) {
    const item = ITEM.exec(lines[i] ?? "");
    if (!item || (item[3] !== undefined) !== ordered) break;
    const indent =
      (item[1]?.length ?? 0) + (item[2]?.length ?? 0) + (item[4] ? 1 : 0);
    const body = [item[5] ?? ""];
    i++;
    while (i < lines.length) {
      const next = lines[i] ?? "";
      if (next.trim() === "") {
        // A blank line ends the list unless the item or list goes on.
        const after = lines[i + 1] ?? "";
        if (leading(after) >= indent || ITEM.test(after)) {
          body.push("");
          i++;
          continue;
        }
        break;
      }
      if (leading(next) >= indent) {
        body.push(next.slice(indent));
      } else if (ITEM.test(next) || interrupts(next)) {
        break;
      } else {
        // A lazy continuation of the item's paragraph.
        body.push(next.trimStart());
      }
      i++;
    }
    items.push(blocks(body));
  }
  out.push({
    kind: "list",
    ordered,
    start: ordered ? Number(first?.[3] ?? 1) : 1,
    items,
  });
  return i;
}

/** Whether `line` starts a block other than a paragraph's next line. */
function interrupts(line: string): boolean {
  return (
    line.trim() === "" ||
    FENCE.test(line) ||
    HEADING.test(line) ||
    RULE.test(line) ||
    QUOTE.test(line) ||
    ITEM.test(line)
  );
}

function closes(line: string, mark: string): boolean {
  const trimmed = line.trim();
  return (
    trimmed.length >= mark.length &&
    trimmed[0] === mark[0] &&
    [...trimmed].every((c) => c === mark[0])
  );
}

function leading(line: string): number {
  return line.length - line.trimStart().length;
}

function cells(line: string): Inline[][] {
  let row = line.trim();
  if (row.startsWith("|")) row = row.slice(1);
  if (row.endsWith("|") && !row.endsWith("\\|")) row = row.slice(0, -1);
  return row
    .split(/(?<!\\)\|/)
    .map((cell) => inlines(cell.trim().replaceAll("\\|", "|")));
}

const ESCAPABLE = "\\`*_{}[]()#+-.!|~<>\"'";
const URL = /^<(https?:\/\/[^\s<>]+)>/;

/** `text`'s spans: code, emphasis, links and images, the rest as text. */
export function inlines(text: string): Inline[] {
  const out: Inline[] = [];
  let plain = "";
  const flush = () => {
    if (plain) out.push({ kind: "text", text: plain });
    plain = "";
  };
  let i = 0;
  while (i < text.length) {
    const c = text[i] ?? "";
    const rest = text.slice(i);
    if (c === "\\" && ESCAPABLE.includes(text[i + 1] ?? "")) {
      plain += text[i + 1];
      i += 2;
      continue;
    }
    if (c === "\\" && text[i + 1] === "\n") {
      flush();
      out.push({ kind: "break" });
      i += 2;
      continue;
    }
    if (c === "`") {
      const run = /^`+/.exec(rest)?.[0] ?? "`";
      const end = closingRun(text, i + run.length, run.length);
      if (end >= 0) {
        flush();
        let code = text.slice(i + run.length, end).replaceAll("\n", " ");
        if (code.length > 2 && code.startsWith(" ") && code.endsWith(" ")) {
          code = code.slice(1, -1);
        }
        out.push({ kind: "code", text: code });
        i = end + run.length;
        continue;
      }
      plain += run;
      i += run.length;
      continue;
    }
    if (c === "!" && text[i + 1] === "[") {
      const link = linkAt(text, i + 1);
      if (link) {
        flush();
        out.push({ kind: "image", alt: link.label });
        i = link.end;
        continue;
      }
    }
    if (c === "[") {
      const link = linkAt(text, i);
      if (link) {
        flush();
        out.push({ kind: "link", href: link.href, text: inlines(link.label) });
        i = link.end;
        continue;
      }
    }
    const url = c === "<" ? URL.exec(rest) : null;
    if (url) {
      flush();
      const href = url[1] ?? "";
      out.push({ kind: "link", href, text: [{ kind: "text", text: href }] });
      i += url[0].length;
      continue;
    }
    const span = emphasisAt(text, i);
    if (span) {
      flush();
      out.push({ kind: span.kind, text: inlines(span.inner) });
      i = span.end;
      continue;
    }
    if (c === "\n") {
      // Two trailing spaces end a line; otherwise a line break is a space.
      if (plain.endsWith("  ")) {
        plain = plain.trimEnd();
        flush();
        out.push({ kind: "break" });
      } else {
        plain += " ";
      }
      i++;
      continue;
    }
    plain += c;
    i++;
  }
  flush();
  return out;
}

/** Where a run of exactly `length` backticks starts, from `from`, or -1. */
function closingRun(text: string, from: number, length: number): number {
  let i = text.indexOf("`", from);
  while (i >= 0) {
    let end = i;
    while (text[end] === "`") end++;
    if (end - i === length) return i;
    i = text.indexOf("`", end);
  }
  return -1;
}

/** `[label](href "title")` at `start`, or undefined. */
function linkAt(
  text: string,
  start: number,
): { label: string; href: string; end: number } | undefined {
  let depth = 0;
  let close = -1;
  for (let i = start; i < text.length; i++) {
    const c = text[i];
    if (c === "\\") {
      i++;
    } else if (c === "[") {
      depth++;
    } else if (c === "]" && --depth === 0) {
      close = i;
      break;
    }
  }
  if (close < 0 || text[close + 1] !== "(") return undefined;
  const target = /^\(\s*<?([^\s()<>]*)>?(?:\s+(?:"[^"]*"|'[^']*'))?\s*\)/.exec(
    text.slice(close + 1),
  );
  if (!target) return undefined;
  return {
    label: text.slice(start + 1, close),
    href: target[1] ?? "",
    end: close + 1 + target[0].length,
  };
}

const DELIMITERS = [
  ["**", "strong"],
  ["__", "strong"],
  ["~~", "del"],
  ["*", "em"],
  ["_", "em"],
] as const;

/**
 * Emphasis opening at `start`: a delimiter followed by text, closed by the
 * same delimiter after text. `_` only counts at a word's edge, so snake_case
 * stays as written.
 */
function emphasisAt(
  text: string,
  start: number,
): { kind: "strong" | "em" | "del"; inner: string; end: number } | undefined {
  for (const [mark, kind] of DELIMITERS) {
    if (!text.startsWith(mark, start)) continue;
    const after = text[start + mark.length] ?? "";
    if (after === "" || /\s/.test(after)) return undefined;
    if (mark[0] === "_" && /\w/.test(text[start - 1] ?? "")) return undefined;
    let close = text.indexOf(mark, start + mark.length + 1);
    while (close >= 0) {
      const before = text[close - 1] ?? "";
      const next = text[close + mark.length] ?? "";
      const edge = mark[0] !== "_" || !/\w/.test(next);
      // A single mark mustn't close on half of a double one.
      const whole = mark.length === 2 || next !== mark[0];
      if (!/\s/.test(before) && before !== "\\" && edge && whole) {
        return {
          kind,
          inner: text.slice(start + mark.length, close),
          end: close + mark.length,
        };
      }
      close = text.indexOf(mark, close + 1);
    }
    return undefined;
  }
  return undefined;
}
