import { type JSX, Show } from "solid-js";
import type { Item } from "../api/Item";
import type { ItemState } from "../api/ItemState";
import { Badge, ExternalLink, Meta, Mono, type Tone } from "../design-system";
import { Counts } from "./counts";
import styles from "./header.module.css";

/** What the item is, where it stands, and how much of it is viewed. */
export function ItemHeader(props: {
  item: Item;
  state: ItemState;
  viewed: number;
  files: number;
  /** A viewed mark the hub refused. */
  failure: string | undefined;
  /** At the end of the header, which the Conversation list hangs below. */
  children?: JSX.Element;
}) {
  return (
    <header class={styles.header}>
      <Mono>{props.item.id}</Mono>
      <h1 class={styles.title}>{props.item.title}</h1>
      <Badge tone="brand">r{props.item.revision.number}</Badge>
      <Badge tone={stateTone[props.state]}>{stateLabel[props.state]}</Badge>
      <Meta>
        <Mono>{props.item.branch}</Mono> → <Mono>{props.item.target}</Mono>
      </Meta>
      <Show when={props.item.pull}>
        {(pull) => (
          <ExternalLink href={pull().url} title="Pull request on GitHub">
            #{pull().number}
          </ExternalLink>
        )}
      </Show>
      <Counts
        additions={props.item.revision.additions}
        deletions={props.item.revision.deletions}
      />
      <Meta>
        {props.viewed}/{props.files} viewed
      </Meta>
      <Show when={props.failure}>
        {(message) => <Meta tone="removed">{message()}</Meta>}
      </Show>
      {props.children}
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
  needs_review: "neutral",
  changes_requested: "warning",
  merging: "brand",
  merged: "success",
  abandoned: "neutral",
};
