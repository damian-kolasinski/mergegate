import { onCleanup } from "solid-js";
import type { ErrorEnvelope } from "../api/ErrorEnvelope";
import type { UiEvent } from "../api/UiEvent";

/** A failed request, carrying the hub's error envelope when it sent one. */
export class HubError extends Error {
  constructor(
    message: string,
    readonly code: string,
  ) {
    super(message);
  }
}

/** `GET`s a hub endpoint. The UI is served by the hub, so paths are relative. */
export async function getJson<T>(path: string): Promise<T> {
  return (await send(path, { headers: { accept: "application/json" } })) as T;
}

/** `PUT`s `body` to a hub endpoint and returns what it answered, if anything. */
export async function putJson<T = void>(
  path: string,
  body: unknown,
): Promise<T> {
  return (await send(path, {
    method: "PUT",
    // The hub refuses a change without it, which a cross-site page can't add.
    headers: { "content-type": "application/json", [clientHeader]: "ui" },
    body: JSON.stringify(body),
  })) as T;
}

/** `POST`s `body` to a hub endpoint and returns what it answered. */
export async function postJson<T>(path: string, body: unknown): Promise<T> {
  return (await send(path, {
    method: "POST",
    headers: { "content-type": "application/json", [clientHeader]: "ui" },
    body: JSON.stringify(body),
  })) as T;
}

/** `DELETE`s a hub resource. */
export async function deleteJson(path: string): Promise<void> {
  await send(path, { method: "DELETE", headers: { [clientHeader]: "ui" } });
}

/** `api::CLIENT_HEADER`. */
const clientHeader = "x-mergegate-client";

async function send(path: string, init: RequestInit): Promise<unknown> {
  let response: Response;
  try {
    response = await fetch(path, init);
  } catch {
    throw new HubError("The hub isn't running.", "hub_unreachable");
  }
  const body: unknown = await response.json().catch(() => undefined);
  if (!response.ok) {
    const error = (body as ErrorEnvelope | undefined)?.error;
    throw new HubError(
      error?.message ?? `The hub answered ${response.status}.`,
      error?.code ?? "unexpected_response",
    );
  }
  return body;
}

type Listener = () => void;

const listeners = new Map<UiEvent["type"], Set<Listener>>();
let source: EventSource | undefined;

/**
 * Calls `listener` whenever the hub says `type` changed, for as long as the
 * calling component lives. It also runs on every (re)connect: events sent
 * while the stream was down are lost, so the page refetches instead.
 */
export function onHubEvent(type: UiEvent["type"], listener: Listener): void {
  let forType = listeners.get(type);
  if (!forType) {
    forType = new Set();
    listeners.set(type, forType);
  }
  forType.add(listener);
  const added = forType;
  onCleanup(() => added.delete(listener));
  connect();
}

// Each open stream holds one of the browser's six HTTP/1.1 connections to
// the hub, so a hidden tab gives its stream up; reopening refetches.
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) {
    connect();
  } else if (source) {
    source.close();
    source = undefined;
  }
});

function connect() {
  const listening = [...listeners.values()].some((forType) => forType.size);
  if (source || document.hidden || !listening) return;
  // One stream for the whole page; EventSource reconnects by itself.
  source = new EventSource("/api/events/ui");
  source.onmessage = (message: MessageEvent<string>) => {
    const event = JSON.parse(message.data) as UiEvent;
    for (const listener of listeners.get(event.type) ?? []) listener();
  };
  source.onopen = () => {
    for (const forType of listeners.values()) {
      for (const listener of forType) listener();
    }
  };
}
