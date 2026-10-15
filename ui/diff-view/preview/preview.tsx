import { Match, onMount, Switch } from "solid-js";
import type { File } from "../../api/File";
import { useModalKeys } from "../../app/keys";
import { Button } from "../../design-system";
import { ImageCompare } from "./image-compare";
import { previewOf } from "./kind";
import styles from "./preview.module.css";

/**
 * A file shown as what it is rather than as lines, over the review. Escape
 * closes it, and the review's keys wait until it's closed.
 */
export function PreviewDialog(props: {
  file: File;
  blobUrl: (id: string) => string;
  onClose: () => void;
}) {
  let close: HTMLButtonElement | undefined;
  useModalKeys({ Escape: () => props.onClose() });
  onMount(() => close?.focus());
  return (
    <div class={styles.scrim} onClick={() => props.onClose()}>
      <div
        class={styles.dialog}
        role="dialog"
        aria-modal="true"
        aria-label={props.file.path}
        onClick={(event) => event.stopPropagation()}
      >
        <div class={styles.bar}>
          <span class={styles.name}>{props.file.path}</span>
          <span class={styles.actions}>
            <Button
              onClick={() => props.onClose()}
              ref={(element) => {
                close = element;
              }}
            >
              Close
            </Button>
          </span>
        </div>
        <div class={styles.body}>
          <Switch>
            <Match when={previewOf(props.file) === "image"}>
              <ImageCompare file={props.file} blobUrl={props.blobUrl} />
            </Match>
          </Switch>
        </div>
      </div>
    </div>
  );
}
