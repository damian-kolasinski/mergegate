import type { File } from "../api/File";

/**
 * Why a file is where it is, in one line: the agent's reason, the judge's
 * score, the rule that tiered it, or failing those the heuristics' hint.
 * The file list and the diff's file header both show it; the list leaves
 * the score out with `score: false`, as it shows it on its own.
 */
export function describe(
  file: File,
  { score = true }: { score?: boolean } = {},
): string | undefined {
  const parts = [
    file.reason,
    judged(file, score),
    file.why && tier(file, file.why),
  ];
  const said = parts.filter((part): part is string => Boolean(part));
  // A judged file is placed by its score, not the heuristics.
  if (!said.length && file.hint && file.importance === null) {
    said.push(file.hint);
  }
  return said.length ? said.join(" · ") : undefined;
}

/** The judge's score out of 100, as the file list's chip titles it. */
export function importance(file: File): string | undefined {
  if (file.importance === null) return undefined;
  const by = file.raised ? "raised by the judge" : "judged";
  return `${by} ${file.importance}/100`;
}

/**
 * The judge's score and line, as in "judged 87/100: touches the expiry";
 * without the score, only what it adds, as in "judge: touches the expiry".
 */
function judged(file: File, score: boolean): string | undefined {
  const head = score
    ? importance(file)
    : file.importance === null
      ? undefined
      : file.raised
        ? "raised by the judge"
        : file.judge_reason && "judge";
  if (!head) return undefined;
  return file.judge_reason ? `${head}: ${file.judge_reason}` : head;
}

/** Which rule tiered the file, as in "trusted by R3: `*.lock` — lockfile". */
function tier(file: File, why: string): string {
  if (!file.rule) return `kept in review: ${why}`;
  if (file.promoted) {
    return `promoted by the judge; trusted by ${file.rule}: ${why}`;
  }
  return `${file.tier === "trusted" ? "trusted" : "kept in review"} by ${file.rule}: ${why}`;
}
