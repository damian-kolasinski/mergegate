import type { ItemDiff } from "../../api/ItemDiff";
import { getJson, onHubEvent } from "../../app/hub";
import type { Drafts } from "./drafts";
import type { Replies } from "./replies";
import type { Sent } from "./sent";

/**
 * Rereads the item's threads when the hub says they changed: the agent
 * replied or submitted, or the stream reconnected and may have missed it.
 */
export function useSync(
  item: string,
  slices: { sent: Sent; drafts: Drafts; replies: Replies },
) {
  let asked = 0;
  const refresh = () => {
    const ask = ++asked;
    getJson<ItemDiff>(`/api/items/${encodeURIComponent(item)}`).then(
      (latest) => {
        // An older answer mustn't replace a newer one.
        if (ask !== asked) return;
        slices.replies.keep(latest.replies);
        // Another tab may have sent drafts this one still shows. Drafts
        // themselves stay as this tab has them, saves in flight included.
        slices.sent.keep(latest.comments);
        slices.drafts.sent(latest.comments);
      },
      // What's shown stays until the next refetch.
      () => undefined,
    );
  };
  onHubEvent("item.changed", (event) => {
    if (!event || event.item === item) refresh();
  });
  return refresh;
}
