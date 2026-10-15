// diff-view/plain-text.ts builds plain text's HAST without Shiki's
// tokenizer; it must stay what Shiki builds, through an upgrade of Pierre
// or Shiki included.

import {
  DEFAULT_THEMES,
  getSharedHighlighter,
  parseDiffFromFile,
  type RenderDiffOptions,
  renderDiffWithHighlighter,
} from "@pierre/diffs";
import { expect, test } from "@playwright/test";
import { withPlainText } from "../diff-view/plain-text";

const lines = [
  "",
  " ",
  "\t{",
  "  foo(bar);  ",
  "a\tb",
  "é 日本 😀",
  "<b>&amp;</b>",
];

/** `count` lines cycling through `lines`, each changed by `salt`. */
const text = (count: number, salt: string, eol = "\n") =>
  Array.from(
    { length: count },
    (_, n) => `${lines[n % lines.length]}${n % 3 ? salt : ""}`,
  ).join(eol) + eol;

test("plain text renders as Shiki renders it", async () => {
  const highlighter = await getSharedHighlighter({
    themes: [DEFAULT_THEMES.dark, DEFAULT_THEMES.light],
    langs: ["text"],
  });
  const options: RenderDiffOptions = {
    theme: DEFAULT_THEMES,
    useTokenTransformer: false,
    tokenizeMaxLineLength: 1000,
    lineDiffType: "word-alt",
    maxLineDiffLength: 1000,
  };
  const pairs = [
    ["", text(3, "x")],
    [text(3, "x"), ""],
    [text(20, "a"), text(22, "b")],
    [text(20, "a", "\r\n"), text(20, "b", "\r\n")],
    [text(20, "a").trimEnd(), text(20, "b").trimEnd()],
    // Above 1,000 lines Pierre drops word diffs, so the fast path runs.
    [text(1500, "a"), text(1500, "b")],
  ];
  for (const [before, after] of pairs) {
    const diff = parseDiffFromFile(
      { name: "a.txt", contents: before ?? "" },
      { name: "a.txt", contents: after ?? "" },
    );
    for (const forcePlainText of [true, false]) {
      const render = (h: typeof highlighter) =>
        JSON.stringify(
          renderDiffWithHighlighter(diff, h, options, { forcePlainText }),
        );
      expect(render(withPlainText(highlighter))).toBe(render(highlighter));
    }
  }
});
