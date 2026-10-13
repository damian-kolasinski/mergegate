import { onCleanup } from "solid-js";

/** Actions by `KeyboardEvent.key`, e.g. `j` or `Enter`. */
export type Keymap = Readonly<Record<string, () => void>>;

/**
 * Binds `keymap` for as long as the calling component lives. Keys typed into
 * a field or pressed with a modifier stay with the browser, so shortcuts
 * never eat text or `cmd-r`.
 */
export function useKeys(keymap: Keymap): void {
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
    const action = keymap[event.key];
    if (!action) return;
    event.preventDefault();
    action();
  };
  window.addEventListener("keydown", onKey);
  onCleanup(() => window.removeEventListener("keydown", onKey));
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
