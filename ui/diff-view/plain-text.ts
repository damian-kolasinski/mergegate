import type { DiffsHighlighter } from "@pierre/diffs";
import {
  isPlainLang,
  type ShikiTransformerContextSource,
  splitLines,
  type ThemedToken,
  tokensToHast,
} from "shiki/core";

type Options = Parameters<DiffsHighlighter["codeToHast"]>[1];

const wrapped = new WeakMap<DiffsHighlighter, DiffsHighlighter>();

/**
 * `highlighter`, but turning plain text into HAST without tokenizing it.
 * Every file renders as plain text, coloured by the hub's spans, and Shiki
 * tokenizes plain text once per theme only to give each line one unstyled
 * token: for a near-cap file that was half of opening it. The tokens built
 * here are those, and Shiki's own `tokensToHast` runs the transformers, so
 * the HAST is the same.
 */
export function withPlainText(highlighter: DiffsHighlighter): DiffsHighlighter {
  let plain = wrapped.get(highlighter);
  if (!plain) {
    plain = Object.create(highlighter) as DiffsHighlighter;
    plain.codeToHast = (code, options) =>
      plainHast(highlighter, code, options) ??
      highlighter.codeToHast(code, options);
    wrapped.set(highlighter, plain);
  }
  return plain;
}

/**
 * What Shiki's `codeToHast` returns for `code`, or nothing where its steps
 * before `tokensToHast` could change a line's one token: decorations split
 * it, as can a transformer's `preprocess` or `tokens`, and splitting
 * whitespace off it. Pierre turns word diffs off above 1,000 plain lines,
 * so a large file has no decorations.
 */
function plainHast(
  highlighter: DiffsHighlighter,
  code: string,
  options: Options,
): ReturnType<DiffsHighlighter["codeToHast"]> | undefined {
  if (
    !isPlainLang(options.lang) ||
    options.decorations?.length ||
    options.mergeWhitespaces === "never" ||
    options.transformers?.some((t) => t.preprocess || t.tokens)
  ) {
    return undefined;
  }
  // An empty line's token carries the themes' styling for every line.
  const { tokens, fg, bg, themeName, rootStyle } = highlighter.codeToTokens(
    "",
    options,
  );
  const template = tokens[0]?.[0];
  if (!template) return undefined;
  const lines: ThemedToken[][] = splitLines(code).map(([content, offset]) => [
    { ...template, content, offset },
  ]);
  const context: ShikiTransformerContextSource = {
    meta: {},
    options,
    codeToHast: (c, o) => highlighter.codeToHast(c, o),
    codeToTokens: (c, o) => highlighter.codeToTokens(c, o),
    get source() {
      return code;
    },
  };
  // As Shiki's `codeToHast` passes them, `fg` and `bg` unset or not.
  const render = {
    ...options,
    fg,
    bg,
    themeName,
    rootStyle:
      options.rootStyle === false ? false : (options.rootStyle ?? rootStyle),
  } as Parameters<typeof tokensToHast>[1];
  return tokensToHast(lines, render, context);
}
