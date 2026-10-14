import { createMemo, Match, Switch } from "solid-js";
import { render } from "solid-js/web";
import { InboxScreen } from "../inbox/inbox";
import { ItemScreen } from "../item/item";
import { RulesScreen } from "../rules/rules";
import { useRoute } from "./router";
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
  return (
    <Shell fill={item() !== undefined}>
      <Switch fallback={<InboxScreen />}>
        <Match when={item()} keyed>
          {(id) => <ItemScreen id={id} />}
        </Match>
        <Match when={project()} keyed>
          {(name) => <RulesScreen project={name} />}
        </Match>
      </Switch>
    </Shell>
  );
}

const root = document.getElementById("root");
if (root) render(() => <App />, root);
