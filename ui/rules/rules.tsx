import { createSignal, For, type JSX, Show, untrack } from "solid-js";
import type { Preview } from "../api/Preview";
import type { Rule } from "../api/Rule";
import type { Rules } from "../api/Rules";
import {
  type Failure,
  failureOf,
  getJson,
  onHubEvent,
  postJson,
} from "../app/hub";
import { useKeys } from "../app/keys";
import { go, inboxHref, itemHref } from "../app/router";
import { useTitle } from "../app/title";
import {
  Badge,
  Button,
  CodeText,
  Empty,
  ErrorNote,
  Meta,
  Mono,
  Section,
} from "../design-system";
import styles from "./rules.module.css";

/**
 * A project's rules and the changes to them waiting for the operator: a
 * trusted rule an agent proposed, or a hand edit that would hide more.
 */
export function RulesScreen(props: {
  project: string;
  /** The item the operator came from, which Escape goes back to. */
  back?: string | undefined;
}) {
  // main.tsx keys the route, so the project is fixed for the screen's life.
  const project = untrack(() => props.project);
  useTitle(() => `${project} rules`);
  const route = `/api/projects/${encodeURIComponent(project)}/rules`;
  const [rules, setRules] = createSignal<Rules>();
  const [failure, setFailure] = createSignal<Failure>();
  // The change being decided, and the one the hub refused, by its path.
  const [busy, setBusy] = createSignal<string>();
  const [refused, setRefused] = createSignal<{ path: string } & Failure>();
  let latest = 0;
  const load = async () => {
    // Responses can arrive out of order; only the newest request's counts.
    const request = ++latest;
    try {
      const next = await getJson<Rules>(route);
      if (request !== latest) return;
      setRules(next);
      setFailure(undefined);
    } catch (error) {
      if (request !== latest) return;
      setFailure(failureOf(error));
    }
  };
  void load();
  onHubEvent("rules.changed", (event) => {
    if (!event || event.project === project) void load();
  });
  // main.tsx keys the route, so where it leads back to is fixed too.
  const back = untrack(() => props.back);
  const leave = () => {
    go(back ? itemHref(back) : inboxHref);
  };
  useKeys(
    { Escape: leave },
    {
      group: "Rules",
      keys: [["Esc", back ? `Back to ${back}` : "Back to the inbox"]],
    },
  );

  const decide = async (path: string) => {
    setBusy(path);
    setRefused(undefined);
    latest++;
    try {
      setRules(await postJson<Rules>(`${route}/${path}`, {}));
      setFailure(undefined);
    } catch (error) {
      setRefused({ path, ...failureOf(error) });
    } finally {
      setBusy(undefined);
    }
  };
  const refusal = (path: string) => {
    const failed = refused();
    return failed && failed.path.startsWith(path) ? failed : undefined;
  };

  return (
    <div class={styles.screen} role="region" aria-label={`${project} rules`}>
      <header class={styles.header}>
        <Show when={props.back}>
          {(back) => (
            <a class={styles.back} href={itemHref(back())}>
              ← back to <Mono>{back()}</Mono>
            </a>
          )}
        </Show>
        <h1 class={styles.title}>{project} rules</h1>
        <Meta>
          Each project has its own; these sort {project}'s items only.
        </Meta>
      </header>
      <Show when={failure()}>
        {(failure) => (
          <Empty>
            <ErrorNote failure={failure()} />
          </Empty>
        )}
      </Show>
      <Show when={rules()}>
        {(current) => (
          <>
            <Show when={current().error}>
              {(error) => (
                <p class={styles.error} role="alert">
                  <Mono>{current().location}</Mono> can't be used, so the rules
                  before it still apply: {error()}
                </p>
              )}
            </Show>
            <Show when={current().pending.length || current().edit}>
              <Section
                title="Waiting for you"
                count={current().pending.length + (current().edit ? 1 : 0)}
              >
                <ul class={styles.changes}>
                  <For each={current().pending}>
                    {(proposal) => (
                      <Change
                        label={`Proposed ${proposal.rule.id}`}
                        title={
                          <>
                            An agent proposed a trusted rule, which goes last:
                          </>
                        }
                        rules={[proposal.rule]}
                        preview={proposal.preview}
                        busy={busy()?.startsWith(
                          `proposals/${proposal.rule.id}/`,
                        )}
                        locked={busy() !== undefined}
                        failure={refusal(`proposals/${proposal.rule.id}/`)}
                        onApprove={() =>
                          void decide(`proposals/${proposal.rule.id}/approve`)
                        }
                        onReject={() =>
                          void decide(`proposals/${proposal.rule.id}/reject`)
                        }
                      />
                    )}
                  </For>
                  <Show when={current().edit}>
                    {(edit) => (
                      <Change
                        label="Edit to the rules file"
                        title={
                          <>
                            <Mono>{current().location}</Mono> changed to these
                            rules, which would hide more; rejecting writes the
                            applied ones back:
                          </>
                        }
                        rules={edit().rules}
                        preview={edit().preview}
                        busy={busy()?.startsWith("edit/")}
                        locked={busy() !== undefined}
                        failure={refusal("edit/")}
                        // Rejecting overwrites a file the operator may have
                        // edited by hand, so it asks first.
                        confirmReject={`Overwrite ${fileName(current().location)} with the ${current().rules.length} applied ${current().rules.length === 1 ? "rule" : "rules"}?`}
                        onApprove={() => void decide("edit/approve")}
                        onReject={() => void decide("edit/reject")}
                      />
                    )}
                  </Show>
                </ul>
              </Section>
            </Show>
            <Section title="Applied rules">
              <Show
                when={current().rules.length}
                fallback={
                  <Empty>
                    No rules, so every file is reviewed. Write them in{" "}
                    <Mono>{current().location}</Mono>.
                  </Empty>
                }
              >
                <p class={styles.note}>
                  First match wins. Edit them in{" "}
                  <Mono>{current().location}</Mono>.
                </p>
                <RuleList label="Applied rules" rules={current().rules} />
              </Show>
            </Section>
          </>
        )}
      </Show>
    </div>
  );
}

function Change(props: {
  label: string;
  title: JSX.Element;
  rules: Rule[];
  preview: Preview | null;
  /** This change is being decided. */
  busy: boolean | undefined;
  /** Some change is: one decision at a time. */
  locked: boolean;
  /** What the hub said to the last decision on this change. */
  failure: Failure | undefined;
  /** Set when rejecting takes a confirm: the question it asks. */
  confirmReject?: string | undefined;
  onApprove: () => void;
  onReject: () => void;
}) {
  const [confirming, setConfirming] = createSignal(false);
  return (
    <li class={styles.change} aria-label={props.label}>
      <p>{props.title}</p>
      <RuleList label="Rules" rules={props.rules} />
      <Show when={props.preview}>
        {(preview) => (
          <>
            <p
              class={styles.preview}
              data-none={preview().hidden.length ? undefined : ""}
            >
              {wouldHide(preview())}
            </p>
            <Show when={preview().hidden.length}>
              <ul class={styles.hidden} aria-label="Files it would have hidden">
                <For each={preview().hidden}>
                  {(file) => (
                    <li>
                      <a href={itemHref(file.item)}>
                        <Mono>{file.item}</Mono>
                      </a>
                      <Mono>{file.path}</Mono>
                      <Meta>
                        {file.title} · by {file.rule}
                      </Meta>
                    </li>
                  )}
                </For>
              </ul>
            </Show>
          </>
        )}
      </Show>
      <div class={styles.actions}>
        <Show
          when={confirming()}
          fallback={
            <>
              <Button
                variant="solid"
                disabled={props.locked}
                busy={props.busy}
                onClick={props.onApprove}
              >
                Approve
              </Button>
              <Button
                disabled={props.locked}
                onClick={() =>
                  props.confirmReject ? setConfirming(true) : props.onReject()
                }
              >
                Reject
              </Button>
            </>
          }
        >
          <span class={styles.question}>{props.confirmReject}</span>
          <Button onClick={() => setConfirming(false)}>Cancel</Button>
          <Button
            variant="danger"
            disabled={props.locked}
            busy={props.busy}
            onClick={() => {
              setConfirming(false);
              props.onReject();
            }}
          >
            Overwrite
          </Button>
        </Show>
        <Show when={props.failure}>
          {(failure) => <ErrorNote failure={failure()} />}
        </Show>
      </div>
    </li>
  );
}

function RuleList(props: { label: string; rules: Rule[] }) {
  return (
    <ol class={styles.rules} aria-label={props.label}>
      <For each={props.rules}>
        {(rule) => (
          <li>
            <Mono>{rule.id}</Mono>
            <Badge tone={rule.tier === "trusted" ? "brand" : "neutral"}>
              {rule.tier}
            </Badge>
            <Mono>{rule.summary}</Mono>
            <span class={styles.reason}>
              <CodeText text={rule.reason} />
            </span>
          </li>
        )}
      </For>
    </ol>
  );
}

/** As in "Would have hidden 3 files in 2 of 14 recent items." */
function wouldHide(preview: Preview): string {
  const files = preview.hidden.length;
  const items = new Set(preview.hidden.map((file) => file.item)).size;
  const recent = preview.items === 1 ? "recent item" : "recent items";
  return `Would have hidden ${files} ${files === 1 ? "file" : "files"} in ${items} of ${preview.items} ${recent}.`;
}

/** `rules.toml` of `.mergegate/rules.toml`. */
function fileName(location: string): string {
  return location.split("/").at(-1) ?? location;
}
