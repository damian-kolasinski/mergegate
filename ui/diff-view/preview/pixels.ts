/** What changed between two images, pixel by pixel. */
export type PixelDiff = {
  /** The new image greyed out, with every changed pixel marked. */
  image: ImageData;
  changed: number;
  total: number;
};

const MARK = [239, 58, 111, 255] as const;

/**
 * Exact RGBA over the larger of both sizes, so a pixel only one image has
 * counts as changed. Unchanged pixels go grey and faint, so a change of a
 * single pixel still stands out.
 */
export function pixelDiff(before: ImageData, after: ImageData): PixelDiff {
  const width = Math.max(before.width, after.width);
  const height = Math.max(before.height, after.height);
  const image = new ImageData(width, height);
  const out = image.data;
  let changed = 0;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const a = at(before, x, y);
      const b = at(after, x, y);
      const o = (y * width + x) * 4;
      if (a < 0 || b < 0 || !samePixel(before.data, a, after.data, b)) {
        changed++;
        out.set(MARK, o);
        continue;
      }
      const d = after.data;
      const grey = ((d[b] ?? 0) + (d[b + 1] ?? 0) + (d[b + 2] ?? 0)) / 3;
      out[o] = out[o + 1] = out[o + 2] = grey;
      out[o + 3] = (d[b + 3] ?? 0) / 4;
    }
  }
  return { image, changed, total: width * height };
}

/** The index of `(x, y)` in `image`'s data, or -1 outside it. */
function at(image: ImageData, x: number, y: number): number {
  return x < image.width && y < image.height ? (y * image.width + x) * 4 : -1;
}

function samePixel(
  a: Uint8ClampedArray,
  i: number,
  b: Uint8ClampedArray,
  j: number,
): boolean {
  return (
    a[i] === b[j] &&
    a[i + 1] === b[j + 1] &&
    a[i + 2] === b[j + 2] &&
    a[i + 3] === b[j + 3]
  );
}
