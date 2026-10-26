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
 * of it is reviewed, with the screen's controls at the end.
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
  return (
    <header class={styles.header}>
      <div class={styles.name}>
        <Mono>{props.item.name}</Mono>
        <h1 class={styles.title} title={props.item.title}>
          {props.item.title}
        </h1>
      </div>
      <div class={styles.facts}>
        <Badge tone="brand" title={`Revision ${props.item.revision.number}`}>
          r{props.item.revision.number}
        </Badge>
        <Badge tone={stateTone[props.state]}>{stateLabel[props.state]}</Badge>
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
        <Meta>
          <Mono>{props.item.branch}</Mono> → <Mono>{props.item.target}</Mono>
        </Meta>
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
        <Meta>
          {props.reviewed} of {props.review} reviewed
          <Show when={props.trusted}> · {props.trusted} trusted</Show>
        </Meta>
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

const stateLabel: Record<ItemState, string> = {
  needs_review: "Needs review",
  changes_requested: "Changes requested",
  merging: "Merging",
  merged: "Merged",
  abandoned: "Abandoned",
};

const stateTone: Record<ItemState, Tone> = {
  needs_review: "brand",
  changes_requested: "warning",
  merging: "brand",
  merged: "success",
  abandoned: "neutral",
};
