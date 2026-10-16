import { createSignal } from "solid-js";

// Routes carry the hub's IDs (`app#3`), but on GitHub the operator knows an
// item by its pull request (`app#84`). The item screen learns each name, so
// what sits around it, like the trail, can say it too.
const [names, setNames] = createSignal<ReadonlyMap<string, string>>(new Map());

/** What the operator calls item `id`, once a screen has loaded it. */
export function nameOf(id: string): string {
  return names().get(id) ?? id;
}

export function learnName(id: string, name: string) {
  if (names().get(id) === name) return;
  setNames(new Map(names()).set(id, name));
}
