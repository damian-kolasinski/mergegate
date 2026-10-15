import { createMemo, createSignal } from "solid-js";
import type { File } from "../api/File";
import type { ItemDiff } from "../api/ItemDiff";
import type { MarkViewed } from "../api/MarkViewed";
import { failureOf, type Failure, putJson } from "../app/hub";

/** The revision's files the operator has marked viewed. */
export function useViewed(diff: () => ItemDiff) {
  // Marks made here, shown before the hub confirms them.
  const [marks, setMarks] = createSignal<ReadonlyMap<string, boolean>>(
    new Map(),
  );
  const [failure, setFailure] = createSignal<Failure>();
  const isViewed = (file: File) => marks().get(file.path) ?? file.viewed;
  const paths = createMemo(
    () =>
      new Set(
        diff()
          .files.filter(isViewed)
          .map((file) => file.path),
      ) as ReadonlySet<string>,
  );
  // A file's marks go out one at a time, so a quick `v v` can't land at the
  // hub in the other order; a refusal takes back only the newest mark, since
  // a later one already says what the operator wants.
  const queues = new Map<string, Promise<unknown>>();
  const newest = new Map<string, number>();
  let sequence = 0;
  return {
    paths,
    /** The last mark the hub refused, which this screen took back. */
    failure,
    mark(path: string, value: boolean) {
      const { item } = diff();
      const body: MarkViewed = {
        revision: item.revision.number,
        path,
        viewed: value,
      };
      const before = paths().has(path);
      const mine = ++sequence;
      newest.set(path, mine);
      setFailure(undefined);
      setMarks((all) => new Map(all).set(path, value));
      const send = () =>
        putJson(`/api/items/${encodeURIComponent(item.id)}/viewed`, body);
      const sent = (queues.get(path) ?? Promise.resolve()).then(send, send);
      queues.set(path, sent);
      sent.catch((error: unknown) => {
        if (newest.get(path) !== mine) return;
        setMarks((all) => new Map(all).set(path, before));
        setFailure(failureOf(error));
      });
    },
  };
}
