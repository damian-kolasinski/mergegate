import { createSignal, For, type JSX, Show, untrack } from "solid-js";
import type { Preview } from "../api/Preview";
import type { Rule } from "../api/Rule";
import type { Rules } from "../api/Rules";
import { getJson, onHubEvent, postJson } from "../app/hub";
import { useKeys } from "../app/keys";
import { inboxHref, itemHref } from "../app/router";
import { Badge, Button, Empty, Meta, Mono, Section } from "../design-system";
import styles from "./rules.module.css";

/**
 * A project's rules and the changes to them waiting for the operator: a
 * trusted rule an agent proposed, or a hand edit that would hide more.
 */
export function RulesScreen(props: { project: string }) {
  // main.tsx keys the route, so the project is fixed for the screen's life.
  const project = untrack(() => props.project);
  const route = `/api/projects/${encodeURIComponent(project)}/rules`;
  const [rules, setRules] = createSignal<Rules>();
  const [failure, setFailure] = createSignal<string>();
  const [busy, setBusy] = createSignal(false);
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
      setFailure(message(error));
    }
  };
  void load();
  onHubEvent("rules.changed", (event) => {
    if (!event || event.project === project) void load();
  });
  useKeys({
    Escape: () => {
      location.hash = inboxHref;
    },
  });

  const decide = async (path: string) => {
    setBusy(true);
    latest++;
    try {
      setRules(await postJson<Rules>(`${route}/${path}`, {}));
      setFailure(undefined);
    } catch (error) {
      setFailure(message(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div class={styles.screen} role="region" aria-label={`${project} rules`}>
      <Show when={failure()}>{(text) => <Empty>{text()}</Empty>}</Show>
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
                        busy={busy()}
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
                        busy={busy()}
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
  busy: boolean;
  onApprove: () => void;
  onReject: () => void;
}) {
  return (
    <li class={styles.change} aria-label={props.label}>
      <p>{props.title}</p>
      <RuleList label="Rules" rules={props.rules} />
      <Show when={props.preview}>
        {(preview) => (
          <>
            <p class={styles.preview}>{wouldHide(preview())}</p>
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
        <Button variant="solid" disabled={props.busy} onClick={props.onApprove}>
          Approve
        </Button>
        <Button disabled={props.busy} onClick={props.onReject}>
          Reject
        </Button>
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
            <span class={styles.reason}>{rule.reason}</span>
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

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
