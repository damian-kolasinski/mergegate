import { createResource, Match, Show, Switch } from "solid-js";
import type { File } from "../../api/File";
import { Meta } from "../../design-system";
import { dimensions } from "./dimensions";
import { type PixelDiff, pixelDiff } from "./pixels";
import styles from "./preview.module.css";

/**
 * A side's decoded pixels take four bytes each, and the tab holds both
 * sides and their diff at once, so past these a side isn't decoded at all.
 */
const MAX_BYTES = 64 * 1024 * 1024;
const MAX_PIXELS = 16_000_000;
/** Past this many pixels both sides are shown but not compared. */
const MAX_COMPARED = 16_000_000;

/** Why a side isn't shown, said to the operator as it is. */
class Refused extends Error {}

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
        <Meta>Can't show this image: {refusal(sides.error)}.</Meta>
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

function refusal(error: unknown): string {
  if (error instanceof Refused) return error.message;
  return "the browser can't decode it";
}

async function load(
  file: File,
  blobUrl: (id: string) => string,
): Promise<Sides> {
  const [before, after] = await Promise.all([
    file.old_blob ? decode("Before", blobUrl(file.old_blob)) : undefined,
    file.new_blob ? decode("After", blobUrl(file.new_blob)) : undefined,
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
 * untyped, and the page's policy allows no `blob:` images. The size comes
 * from the header first, so a small file claiming a huge image is refused
 * before the browser allocates its pixels.
 */
async function decode(side: string, url: string): Promise<ImageData> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url} answered ${response.status}`);
  const length = Number(response.headers.get("content-length"));
  if (length > MAX_BYTES) throw tooManyBytes(side, length);
  const blob = await response.blob();
  if (blob.size > MAX_BYTES) throw tooManyBytes(side, blob.size);
  const size = dimensions(new Uint8Array(await blob.arrayBuffer()));
  if (!size) throw new Refused(`${side} has no PNG, JPEG, GIF or WebP header`);
  if (size.width * size.height > MAX_PIXELS) {
    throw new Refused(
      `${side} is ${size.width} × ${size.height} pixels, past the ${MAX_PIXELS / 1_000_000}-megapixel limit`,
    );
  }
  const bitmap = await createImageBitmap(blob);
  const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
  const context = canvas.getContext("2d");
  if (!context) throw new Error("no 2D canvas");
  context.drawImage(bitmap, 0, 0);
  bitmap.close();
  return context.getImageData(0, 0, canvas.width, canvas.height);
}

function tooManyBytes(side: string, bytes: number): Refused {
  const mb = (n: number) => Math.round(n / (1024 * 1024));
  return new Refused(
    `${side} is ${mb(bytes)} MB, past the ${mb(MAX_BYTES)} MB limit`,
  );
}
