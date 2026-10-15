import { type Accessor, createSignal, onCleanup } from "solid-js";

// Hash routes leave every real path to the hub's API and git endpoints.
export type Route =
  | { screen: "inbox" }
  | { screen: "item"; id: string }
  | { screen: "rules"; project: string };

export function itemHref(id: string): string {
  // Ids carry a `#` (`app#42`), which would end the fragment's path.
  return `#/items/${encodeURIComponent(id)}`;
}

export function rulesHref(project: string): string {
  return `#/projects/${encodeURIComponent(project)}/rules`;
}

export const inboxHref = "#/";

const [route, setRoute] = createSignal(parse(location.hash));

/**
 * Goes to `href` now. `hashchange` fires only after the next frame, so
 * waiting for it would hold back opening an item, and its fetch, by up to a
 * frame. Links and history still arrive through it.
 */
export function go(href: string) {
  location.hash = href;
  setRoute(parse(location.hash));
}

export function useRoute(): Accessor<Route> {
  const onChange = () => setRoute(parse(location.hash));
  window.addEventListener("hashchange", onChange);
  onCleanup(() => window.removeEventListener("hashchange", onChange));
  return route;
}

function parse(hash: string): Route {
  const id = /^#\/items\/(.+)$/.exec(hash)?.[1];
  const project = /^#\/projects\/([^/]+)\/rules$/.exec(hash)?.[1];
  try {
    if (id !== undefined) return { screen: "item", id: decodeURIComponent(id) };
    if (project !== undefined) {
      return { screen: "rules", project: decodeURIComponent(project) };
    }
  } catch {
    // A malformed escape addresses nothing; fall back to the inbox.
  }
  return { screen: "inbox" };
}
