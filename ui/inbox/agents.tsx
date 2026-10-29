import { createSignal, For, Show } from "solid-js";
import type { AgentsStatus } from "../api/AgentsStatus";
import type { SetupReport } from "../api/SetupReport";
import { type Failure, failureOf, getJson, postJson } from "../app/hub";
import { Button, ErrorNote } from "../design-system";
import styles from "./agents.module.css";

/**
 * Offers to set up the coding agents on the hub's machine that don't load
 * mergegate's skill yet, as `mg skill setup` does: a session in one of them
 * would push and open pull requests instead of submitting. The hub answers
 * only its own machine, so on a phone or another device this shows nothing.
 */
export function AgentsPrompt() {
  const [status, setStatus] = createSignal<AgentsStatus>();
  const [busy, setBusy] = createSignal(false);
  const [report, setReport] = createSignal<SetupReport>();
  const [failure, setFailure] = createSignal<Failure>();
  const load = async () => {
    try {
      setStatus(await getJson<AgentsStatus>("/api/agents"));
    } catch {
      setStatus(undefined);
    }
  };
  void load();

  // Only what setup fixes: a skill that isn't mg's, or settings it can't
  // read, would keep the prompt up however often it ran.
  const unready = () =>
    status()?.agents.filter(
      (agent) =>
        agent.found &&
        [agent.skill.state, agent.hook.state].some(
          (state) => state === "missing" || state === "outdated",
        ),
    ) ?? [];
  const setUp = async () => {
    setBusy(true);
    setFailure(undefined);
    try {
      setReport(await postJson<SetupReport>("/api/agents/setup", {}));
      await load();
    } catch (error) {
      setFailure(failureOf(error));
    } finally {
      setBusy(false);
    }
  };
  // What the operator still does: trust a new hook, or fix what failed.
  const notes = () => {
    const steps = report()?.steps ?? [];
    return [
      ...new Set(
        steps.flatMap((step) => [
          ...(step.approve ? [`To finish, ${step.approve}.`] : []),
          ...(step.change === "failed" || step.change === "conflict"
            ? [
                `${step.path ?? step.kind}: ${step.reason ?? "not written by mg, left as it was"}`,
              ]
            : []),
        ]),
      ),
    ];
  };

  return (
    <>
      <Show when={unready().length}>
        <div class={styles.prompt} role="region" aria-label="Agent setup">
          <span class={styles.text}>
            {names(unready().map((agent) => agent.label))}{" "}
            {unready().length === 1 ? "doesn't" : "don't"} load mergegate's
            skill yet, so {unready().length === 1 ? "its" : "their"} sessions
            won't submit here.
          </span>
          <Button variant="solid" busy={busy()} onClick={() => void setUp()}>
            Set up agents
          </Button>
          <Show when={failure()}>
            {(current) => <ErrorNote failure={current()} />}
          </Show>
        </div>
      </Show>
      <Show when={notes().length}>
        <ul class={styles.notes} aria-label="Agent setup notes">
          <For each={notes()}>{(note) => <li>{note}</li>}</For>
        </ul>
      </Show>
    </>
  );
}

/** `Claude Code`, `Claude Code and Codex`, `A, B and C`. */
function names(labels: string[]): string {
  if (labels.length < 2) return labels.join("");
  return `${labels.slice(0, -1).join(", ")} and ${labels.at(-1)}`;
}
