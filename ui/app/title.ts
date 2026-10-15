import { createEffect } from "solid-js";

/**
 * Names the browser tab after what the screen shows, so a tab among many
 * says which review it holds.
 */
export function useTitle(title: () => string | undefined): void {
  createEffect(() => {
    const said = title();
    document.title = said ? `${said} · mergegate` : "mergegate";
  });
}
