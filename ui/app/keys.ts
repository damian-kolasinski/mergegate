import { type Accessor, createSignal, onCleanup } from "solid-js";

/** Actions by `KeyboardEvent.key`, e.g. `j` or `Enter`. */
export type Keymap = Readonly<Record<string, () => void>>;

/**
 * What a screen's keys do, for the `?` overlay: a heading, then each key as
 * it's written on screen (`j / k`, `⌘↩`) with what it does.
 */
export type KeyHelp = {
  group: string;
  keys: readonly (readonly [key: string, does: string])[];
};

// Every live component's help, in the order they bound their keys, so the
// overlay lists what works on the screen as it is now.
const [helps, setHelps] = createSignal<readonly KeyHelp[]>([]);

/** The help of every keymap bound right now. */
export const activeKeyHelp: Accessor<readonly KeyHelp[]> = helps;

/**
 * Binds `keymap` for as long as the calling component lives, listing `help`
 * in the `?` overlay meanwhile. Keys typed into a field or pressed with a
 * modifier stay with the browser, so shortcuts never eat text or `cmd-r`.
 */
export function useKeys(keymap: Keymap, help?: KeyHelp): void {
  listen(keymap, false);
  if (help) describe(help);
}

/**
 * As `useKeys`, for a dialog: its keys come before every screen's, and the
 * screens hear none while it lives, so `Enter` or `a` can't act behind it.
 */
export function useModalKeys(keymap: Keymap, help?: KeyHelp): void {
  listen(keymap, true);
  if (help) describe(help);
}

function describe(help: KeyHelp): void {
  setHelps((all) => [...all, help]);
  onCleanup(() => setHelps((all) => all.filter((other) => other !== help)));
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
    // Enter on a dialog's focused control, such as the demo viewer's Close,
    // is that control's: taken here it would act on the screen behind
    // instead. Elsewhere a link or button left focused by a click keeps the
    // screen's Enter, and a dialog's own keys still decide, as the priority
    // picker's do.
    if (!modal && event.key === "Enter" && inDialogControl(event.target)) {
      return;
    }
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

export function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.isContentEditable ||
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement
  );
}

function inDialogControl(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    target.closest("button, a[href], summary") !== null &&
    target.closest('dialog, [role="dialog"]') !== null
  );
}

/** `⌘↩` (`ctrl-↩` elsewhere), which sends what a field holds. */
export function isSubmitKey(event: KeyboardEvent): boolean {
  return event.key === "Enter" && (event.metaKey || event.ctrlKey);
}
