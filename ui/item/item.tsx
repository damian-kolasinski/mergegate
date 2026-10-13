import { useKeys } from "../app/keys";
import { inboxHref } from "../app/router";
import { Empty, Section } from "../design-system";

/** Where `enter` on the inbox lands. The review screen replaces it. */
export function ItemScreen(props: { id: string }) {
  useKeys({
    Escape: () => {
      location.hash = inboxHref;
    },
  });
  return (
    <Section title={props.id}>
      <Empty>
        Reviewing items isn't built yet. <a href={inboxHref}>Back to inbox</a>
      </Empty>
    </Section>
  );
}
