import type { ChangeContent, FileDiffMetadata } from "@pierre/diffs";

// Ported from @pierre/diffs 1.5.2 (utils/realignChangeContent.js), which
// doesn't export it; `processFile` runs it last. Keep in step on upgrades.
const MAX_ALIGNMENT_COMPARISONS = 4096;
const MIN_IMPROVEMENT_PER_PAIR = 0.5;

/**
 * Splits each change block with more lines on one side than the other so
 * the deletions and additions shown side by side are the most alike, not
 * just the first ones.
 */
export function realign(diff: FileDiffMetadata) {
  for (const hunk of diff.hunks) {
    const content = hunk.hunkContent;
    for (let index = 0; index < content.length; index++) {
      const block = content[index];
      if (block?.type !== "change") continue;
      const replacement = realignBlock(diff, block);
      if (replacement) {
        content.splice(index, 1, ...replacement);
        index += replacement.length - 1;
      }
    }
  }
}

function realignBlock(
  diff: FileDiffMetadata,
  block: ChangeContent,
): ChangeContent[] | undefined {
  const { deletions, additions, deletionLineIndex, additionLineIndex } = block;
  const parts = alignedParts(
    Array.from(
      { length: deletions },
      (_, i) => diff.deletionLines[deletionLineIndex + i] ?? "",
    ),
    Array.from(
      { length: additions },
      (_, i) => diff.additionLines[additionLineIndex + i] ?? "",
    ),
  );
  return parts?.map((part) => ({
    type: "change",
    deletions: part.deletions,
    additions: part.additions,
    deletionLineIndex: deletionLineIndex + part.deletion,
    additionLineIndex: additionLineIndex + part.addition,
  }));
}

/**
 * A piece of a split change block: `deletions` of its removed lines from the
 * `deletion`th, then `additions` of its added ones from the `addition`th.
 */
export type Part = {
  deletions: number;
  additions: number;
  deletion: number;
  addition: number;
};

/**
 * The pieces `realign` splits a change block of `deleted` and `added` lines
 * into, in display order; undefined when it leaves the block whole. The
 * line cursor walks the same pieces, so it moves in the order lines show.
 */
export function alignedParts(
  deleted: readonly string[],
  added: readonly string[],
): Part[] | undefined {
  const deletions = deleted.length;
  const additions = added.length;
  const pairs = Math.min(deletions, additions);
  const surplus = Math.abs(additions - deletions);
  if (
    pairs === 0 ||
    surplus === 0 ||
    pairs * (surplus + 1) > MAX_ALIGNMENT_COMPARISONS
  ) {
    return undefined;
  }
  const deletedText = deleted.map(stripWhitespace);
  const addedText = added.map(stripWhitespace);
  const addsLonger = additions > deletions;
  let bestOffset = 0;
  let bestScore = -1;
  for (let offset = 0; offset <= surplus; offset++) {
    let score = 0;
    for (let pair = 0; pair < pairs; pair++) {
      score += similarity(
        deletedText[pair + (addsLonger ? 0 : offset)] ?? "",
        addedText[pair + (addsLonger ? offset : 0)] ?? "",
      );
    }
    if (offset === 0) bestScore = score + pairs * MIN_IMPROVEMENT_PER_PAIR;
    else if (score > bestScore) {
      bestScore = score;
      bestOffset = offset;
    }
  }
  if (bestOffset === 0) return undefined;
  const parts: Part[] = [];
  const push = (dels: number, adds: number, delAt: number, addAt: number) => {
    if (dels > 0 || adds > 0) {
      parts.push({
        deletions: dels,
        additions: adds,
        deletion: delAt,
        addition: addAt,
      });
    }
  };
  if (addsLonger) {
    push(0, bestOffset, 0, 0);
    push(pairs, pairs, 0, bestOffset);
    push(0, additions - pairs - bestOffset, pairs, bestOffset + pairs);
  } else {
    push(bestOffset, 0, 0, 0);
    push(pairs, pairs, bestOffset, 0);
    push(deletions - pairs - bestOffset, 0, bestOffset + pairs, pairs);
  }
  return parts;
}

function stripWhitespace(line: string) {
  return line.replace(/\s+/g, "");
}

function similarity(a: string, b: string) {
  if (a === b) return 1;
  const max = Math.max(a.length, b.length);
  const min = Math.min(a.length, b.length);
  if (min === 0) return 0;
  let prefix = 0;
  while (prefix < min && a[prefix] === b[prefix]) prefix++;
  let suffix = 0;
  while (
    suffix < min - prefix &&
    a[a.length - 1 - suffix] === b[b.length - 1 - suffix]
  ) {
    suffix++;
  }
  return (prefix + suffix) / max;
}
