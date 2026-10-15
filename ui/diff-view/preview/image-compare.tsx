import { createResource, Match, Show, Switch } from "solid-js";
import type { File } from "../../api/File";
import { Meta } from "../../design-system";
import { type PixelDiff, pixelDiff } from "./pixels";
import styles from "./preview.module.css";

/** Past this many pixels a side is shown but not compared. */
const MAX_COMPARED = 16_000_000;

type Sides = {
  before?: ImageData;
  after?: ImageData;
  diff?: PixelDiff;
};

/** A changed image's two sides and, when it has both, what changed. */
export function ImageCompare(props: {
  file: File;
  blobUrl: (id: string) => string;
}) {
  const [sides] = createResource(
    () => props.file,
    (file) => load(file, props.blobUrl),
  );
  return (
    <Switch fallback={<Meta>Loading…</Meta>}>
      <Match when={sides.error as unknown}>
        <Meta>Can't show this image: the browser can't decode it.</Meta>
      </Match>
      <Match when={sides()}>
        {(loaded) => (
          <div class={styles.panes}>
            <Show when={loaded().before}>
              {(image) => <Pane label="Before" image={image()} />}
            </Show>
            <Show when={loaded().after}>
              {(image) => <Pane label="After" image={image()} />}
            </Show>
            <Show when={loaded().diff}>
              {(diff) => (
                <Pane
                  label="Changes"
                  image={diff().image}
                  note={changedNote(diff())}
                />
              )}
            </Show>
            <Show
              when={
                loaded().before && loaded().after && loaded().diff === undefined
              }
            >
              <Meta>Too large to compare pixel by pixel.</Meta>
            </Show>
          </div>
        )}
      </Match>
    </Switch>
  );
}

function Pane(props: { label: string; image: ImageData; note?: string }) {
  return (
    <figure class={styles.pane}>
      <canvas
        class={styles.canvas}
        role="img"
        aria-label={props.label}
        // Sized here, not as attributes: Solid sets those after the ref, and
        // resizing a canvas clears what it drew.
        ref={(canvas) => {
          canvas.width = props.image.width;
          canvas.height = props.image.height;
          canvas.getContext("2d")?.putImageData(props.image, 0, 0);
        }}
      />
      <figcaption class={styles.caption}>
        <span>{props.label}</span>
        <Meta>
          {props.note ?? `${props.image.width} × ${props.image.height}`}
        </Meta>
      </figcaption>
    </figure>
  );
}

function changedNote(diff: PixelDiff): string {
  if (diff.changed === 0) return "No pixel changed";
  const share = ((diff.changed / diff.total) * 100).toFixed(2);
  return `${diff.changed} of ${diff.total} pixels changed (${share}%)`;
}

async function load(
  file: File,
  blobUrl: (id: string) => string,
): Promise<Sides> {
  const [before, after] = await Promise.all([
    file.old_blob ? decode(blobUrl(file.old_blob)) : undefined,
    file.new_blob ? decode(blobUrl(file.new_blob)) : undefined,
  ]);
  const sides: Sides = {};
  if (before) sides.before = before;
  if (after) sides.after = after;
  if (
    before &&
    after &&
    Math.max(before.width, after.width) *
      Math.max(before.height, after.height) <=
      MAX_COMPARED
  ) {
    sides.diff = pixelDiff(before, after);
  }
  return sides;
}

/**
 * Decoded from the bytes rather than shown from a URL: the hub serves blobs
 * untyped, and the page's policy allows no `blob:` images.
 */
async function decode(url: string): Promise<ImageData> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url} answered ${response.status}`);
  const bitmap = await createImageBitmap(await response.blob());
  const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
  const context = canvas.getContext("2d");
  if (!context) throw new Error("no 2D canvas");
  context.drawImage(bitmap, 0, 0);
  bitmap.close();
  return context.getImageData(0, 0, canvas.width, canvas.height);
}
