import { createMemo, createSignal } from "solid-js";
import type { File } from "../api/File";
import type { ItemDiff } from "../api/ItemDiff";
import type { MarkViewed } from "../api/MarkViewed";
import { putJson } from "../app/hub";

/** The revision's files the operator has marked viewed. */
export function useViewed(diff: () => ItemDiff) {
  // Marks made here, shown before the hub confirms them.
  const [marks, setMarks] = createSignal<ReadonlyMap<string, boolean>>(
    new Map(),
  );
  const [failure, setFailure] = createSignal<string>();
  const isViewed = (file: File) => marks().get(file.path) ?? file.viewed;
  const paths = createMemo(
    () =>
      new Set(
        diff()
          .files.filter(isViewed)
          .map((file) => file.path),
      ) as ReadonlySet<string>,
  );
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
      setFailure(undefined);
      setMarks((all) => new Map(all).set(path, value));
      putJson(`/api/items/${encodeURIComponent(item.id)}/viewed`, body).catch(
        (error: unknown) => {
          setMarks((all) => new Map(all).set(path, !value));
          setFailure(error instanceof Error ? error.message : String(error));
        },
      );
    },
  };
}
