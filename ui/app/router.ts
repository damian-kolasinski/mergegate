import { type Accessor, createSignal, onCleanup } from "solid-js";

// Hash routes leave every real path to the hub's API and git endpoints.
export type Route = { screen: "inbox" } | { screen: "item"; id: string };

export function itemHref(id: string): string {
  // Ids carry a `#` (`app#42`), which would end the fragment's path.
  return `#/items/${encodeURIComponent(id)}`;
}

export const inboxHref = "#/";

export function useRoute(): Accessor<Route> {
  const [route, setRoute] = createSignal(parse(location.hash));
  const onChange = () => setRoute(parse(location.hash));
  window.addEventListener("hashchange", onChange);
  onCleanup(() => window.removeEventListener("hashchange", onChange));
  return route;
}

function parse(hash: string): Route {
  const id = /^#\/items\/(.+)$/.exec(hash)?.[1];
  if (id === undefined) return { screen: "inbox" };
  try {
    return { screen: "item", id: decodeURIComponent(id) };
  } catch {
    return { screen: "inbox" };
  }
}
