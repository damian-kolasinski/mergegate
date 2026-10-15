import { onCleanup } from "solid-js";

/** Actions by `KeyboardEvent.key`, e.g. `j` or `Enter`. */
export type Keymap = Readonly<Record<string, () => void>>;

/**
 * Binds `keymap` for as long as the calling component lives. Keys typed into
 * a field or pressed with a modifier stay with the browser, so shortcuts
 * never eat text or `cmd-r`.
 */
export function useKeys(keymap: Keymap): void {
  listen(keymap, false);
}

/**
 * As `useKeys`, for a dialog: its keys come before every screen's, and the
 * screens hear none while it lives, so `Enter` or `a` can't act behind it.
 */
export function useModalKeys(keymap: Keymap): void {
  listen(keymap, true);
}

function listen(keymap: Keymap, modal: boolean): void {
  const onKey = (event: KeyboardEvent) => {
    if (
      event.defaultPrevented ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey
    ) {
      return;
    }
    if (isEditable(event.target)) return;
    // Capturing on window runs before any listener elsewhere; stopping here
    // keeps the key from them but not from its default, such as `Tab`.
    if (modal) event.stopPropagation();
    const action = keymap[event.key];
    if (!action) return;
    event.preventDefault();
    action();
  };
  window.addEventListener("keydown", onKey, modal);
  onCleanup(() => window.removeEventListener("keydown", onKey, modal));
}

function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.isContentEditable ||
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement
  );
}
