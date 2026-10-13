import { render } from "solid-js/web";
import { Empty } from "../design-system";
import { Shell } from "./shell";

function App() {
  return (
    <Shell>
      <Empty>Nothing to review yet.</Empty>
    </Shell>
  );
}

const root = document.getElementById("root");
if (root) render(() => <App />, root);
