import {
  createEffect,
  createMemo,
  createSignal,
  Match,
  Show,
  Switch,
} from "solid-js";
import { render } from "solid-js/web";
import { InboxScreen } from "../inbox/inbox";
import { ItemScreen } from "../item/item";
import { RulesScreen } from "../rules/rules";
import { inboxHref, itemHref, useRoute } from "./router";
import { Shell } from "./shell";

function App() {
  const route = useRoute();
  const item = createMemo(() => {
    const current = route();
    return current.screen === "item" ? current.id : undefined;
  });
  const project = createMemo(() => {
    const current = route();
    return current.screen === "rules" ? current.project : undefined;
  });
  // The item the operator last had open, so its project's rules lead back
  // to it rather than to the inbox.
  const [lastItem, setLastItem] = createSignal<string>();
  createEffect(() => {
    const id = item();
    if (id !== undefined) setLastItem(id);
  });
  const back = () => {
    const id = lastItem();
    return id?.split("#")[0] === project() ? id : undefined;
  };
  return (
    <Shell
      fill={item() !== undefined}
      trail={
        <Show when={item() ?? project()}>
          <a href={inboxHref}>Inbox</a>
          <span aria-hidden="true">/</span>
          <Show when={item()} fallback={<span>{project()} rules</span>}>
            {(id) => (
              <a href={itemHref(id())} aria-current="page">
                {id()}
              </a>
            )}
          </Show>
        </Show>
      }
    >
      <Switch fallback={<InboxScreen />}>
        <Match when={item()} keyed>
          {(id) => <ItemScreen id={id} />}
        </Match>
        <Match when={project()} keyed>
          {(name) => <RulesScreen project={name} back={back()} />}
        </Match>
      </Switch>
    </Shell>
  );
}

const root = document.getElementById("root");
if (root) render(() => <App />, root);
