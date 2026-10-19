import { For, Match, Switch } from "solid-js";
import type { BotReview } from "../api/BotReview";
import { Badge } from "../design-system";
import styles from "./bot-reviews.module.css";

/**
 * What review bots say about the item's pull request, read from their
 * reactions on its description: 👀 while one reviews, 👍 once it found
 * nothing. Findings come as comments instead, in the Conversation.
 */
export function BotReviews(props: { bots: readonly BotReview[] }) {
  return (
    <For each={props.bots}>
      {(bot) => (
        <Switch>
          <Match when={bot.state === "reviewing"}>
            <Badge
              tone="brand"
              title={`${bot.name} is reviewing the pull request (👀 on GitHub)`}
            >
              <span class={styles.label} data-bot-review="reviewing">
                <svg
                  class={styles.reviewing}
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z" />
                  <circle cx="12" cy="12" r="3" />
                </svg>
                {bot.name} reviewing
              </span>
            </Badge>
          </Match>
          <Match when={bot.state === "approved"}>
            <Badge
              tone="success"
              title={`${bot.name} reviewed the pull request and found nothing (👍 on GitHub)`}
            >
              <span class={styles.label} data-bot-review="approved">
                <svg class={styles.icon} viewBox="0 0 24 24" aria-hidden="true">
                  <path d="M7 10v11H3V10zm0 0 4-8a3 3 0 0 1 3 3v4h5.6a2 2 0 0 1 2 2.3l-1.4 8a2 2 0 0 1-2 1.7H7" />
                </svg>
                {bot.name} approved
              </span>
            </Badge>
          </Match>
        </Switch>
      )}
    </For>
  );
}
