import type { File } from "../api/File";

/**
 * Why a file is where it is, in one line: the agent's reason, the judge's
 * score, the rule that tiered it, or failing those the heuristics' hint.
 * The file list and the diff's file header both show it.
 */
export function describe(file: File): string | undefined {
  const parts = [file.reason, judged(file), file.why && tier(file, file.why)];
  const said = parts.filter((part): part is string => Boolean(part));
  if (!said.length && file.hint) said.push(file.hint);
  return said.length ? said.join(" · ") : undefined;
}

/** The judge's score and line, as in "judged 87/100: touches the expiry". */
function judged(file: File): string | undefined {
  if (file.importance === null) return undefined;
  const score = `${file.raised ? "raised by the judge" : "judged"} ${file.importance}/100`;
  return file.judge_reason ? `${score}: ${file.judge_reason}` : score;
}

/** Which rule tiered the file, as in "trusted by R3: `*.lock` — lockfile". */
function tier(file: File, why: string): string {
  if (!file.rule) return `kept in review: ${why}`;
  if (file.promoted) {
    return `promoted by the judge; trusted by ${file.rule}: ${why}`;
  }
  return `${file.tier === "trusted" ? "trusted" : "kept in review"} by ${file.rule}: ${why}`;
}
