import { type JSX, Show } from "solid-js";
import type { Blocked } from "../api/Blocked";
import type { BotReview } from "../api/BotReview";
import type { Item } from "../api/Item";
import type { ItemState } from "../api/ItemState";
import type { Mergeable } from "../api/Mergeable";
import type { Priority } from "../api/Priority";
import type { StackEntry } from "../api/StackEntry";
import type { Failure } from "../app/hub";
import {
  Badge,
  ErrorNote,
  ExternalLink,
  Meta,
  Mono,
  type Tone,
} from "../design-system";
import { PriorityBadge } from "../priority/priority";
import { BotReviews } from "./bot-reviews";
import { Counts } from "./counts";
import styles from "./header.module.css";
import { MergeableBadge } from "./mergeable";
import { StackTrail } from "./stack";

/**
 * What the item is on a line of its own, then where it stands and how much
 * of it is reviewed, with the screen's controls at the end. Only what tells
 * the operator something shows: the state once it isn't the usual Needs
 * review, the reviewed count once a file is marked; the branch is in the
 * name's tooltip.
 */
export function ItemHeader(props: {
  item: Item;
  state: ItemState;
  /** The item's, or what the operator just set. */
  priority: Priority | null;
  note: string | null;
  /** Files to review that are marked viewed, of `review`. */
  reviewed: number;
  review: number;
  /** Files the rules trust, which need no review. */
  trusted: number;
  /** A viewed mark the hub refused. */
  failure: Failure | undefined;
  /** As the hub reads it: empty when the item isn't stacked. */
  stack: readonly StackEntry[];
  behind: boolean;
  /** Review bots' say on the pull request, beside its link. */
  bots: readonly BotReview[];
  mergeable: Mergeable;
  /** Why it can't merge, which says a conflict in place of `mergeable`. */
  blocked: Blocked | null;
  /** At the end of the second line, which the Conversation list hangs below. */
  children?: JSX.Element;
}) {
  const branch = () => `${props.item.branch} → ${props.item.target}`;
  return (
    <header class={styles.header}>
      <div class={styles.name}>
        <span title={branch()}>
          <Mono>{props.item.name}</Mono>
        </span>
        <h1 class={styles.title} title={`${props.item.title}\n${branch()}`}>
          {props.item.title}
        </h1>
      </div>
      <div class={styles.facts}>
        <Badge tone="brand" title={`Revision ${props.item.revision.number}`}>
          r{props.item.revision.number}
        </Badge>
        <Show when={told(props.state)}>
          {(state) => (
            <Badge tone={stateTone[state()]}>{stateLabel[state()]}</Badge>
          )}
        </Show>
        <MergeableBadge
          mergeable={props.mergeable}
          blocked={props.blocked}
          target={props.item.target}
        />
        <PriorityBadge
          priority={props.priority}
          note={props.note}
          keyHint="P"
        />
        <Show when={props.item.link}>
          {(link) => (
            <ExternalLink href={link().url} title={link().title}>
              {link().label}
            </ExternalLink>
          )}
        </Show>
        <BotReviews bots={props.bots} />
        <Counts
          additions={props.item.revision.additions}
          deletions={props.item.revision.deletions}
        />
        <Show when={props.reviewed || props.trusted}>
          <Meta>
            <Show when={props.reviewed}>
              {props.reviewed} of {props.review} reviewed
            </Show>
            <Show when={props.reviewed && props.trusted}> · </Show>
            <Show when={props.trusted}>{props.trusted} trusted</Show>
          </Meta>
        </Show>
        <Show when={props.failure}>
          {(failure) => <ErrorNote failure={failure()} />}
        </Show>
        <span class={styles.controls}>{props.children}</span>
      </div>
      <Show when={props.stack.length}>
        <StackTrail
          item={props.item.id}
          stack={props.stack}
          behind={props.behind}
        />
      </Show>
    </header>
  );
}

/** A state the header names: all but the one nearly every review is in. */
type Told = Exclude<ItemState, "needs_review">;

function told(state: ItemState): Told | undefined {
  return state === "needs_review" ? undefined : state;
}

const stateLabel: Record<Told, string> = {
  changes_requested: "Changes requested",
  merging: "Merging",
  merged: "Merged",
  abandoned: "Abandoned",
};

const stateTone: Record<Told, Tone> = {
  changes_requested: "warning",
  merging: "brand",
  merged: "success",
  abandoned: "neutral",
};
