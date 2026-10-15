/** An image's size in pixels. */
export type Dimensions = { width: number; height: number };

/**
 * The size a PNG, JPEG, GIF or WebP states in its header, or undefined for
 * anything else. Read before decoding, since a file of a few kilobytes can
 * claim a size whose pixels would take the tab's memory.
 */
export function dimensions(bytes: Uint8Array): Dimensions | undefined {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const found = png(bytes, view) ?? gif(bytes, view) ?? jpeg(bytes, view);
  return found ?? webp(bytes, view);
}

function png(bytes: Uint8Array, view: DataView): Dimensions | undefined {
  const signature = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  if (!starts(bytes, signature, 0) || !ascii(bytes, 12, "IHDR")) {
    return undefined;
  }
  if (bytes.length < 24) return undefined;
  return sized(view.getUint32(16), view.getUint32(20));
}

/** The logical screen, which every frame is drawn within. */
function gif(bytes: Uint8Array, view: DataView): Dimensions | undefined {
  if (!ascii(bytes, 0, "GIF8") || bytes.length < 10) return undefined;
  return sized(view.getUint16(6, true), view.getUint16(8, true));
}

/**
 * The first start-of-frame segment's. Every segment before it says how long
 * it is, so an Exif thumbnail inside one is skipped, not taken for the image.
 */
function jpeg(bytes: Uint8Array, view: DataView): Dimensions | undefined {
  if (!starts(bytes, [0xff, 0xd8], 0)) return undefined;
  let i = 2;
  while (i + 1 < bytes.length) {
    if (bytes[i] !== 0xff) return undefined;
    const marker = bytes[i + 1] ?? 0;
    // Fill bytes, then markers that stand alone, without a length.
    if (marker === 0xff) {
      i += 1;
      continue;
    }
    if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd8)) {
      i += 2;
      continue;
    }
    // The end of the image, or the start of its data, before any frame.
    if (marker === 0xd9 || marker === 0xda || i + 4 > bytes.length) {
      return undefined;
    }
    const length = view.getUint16(i + 2);
    // C4, C8 and CC share the range but aren't frames.
    const frame =
      marker >= 0xc0 && marker <= 0xcf && ![0xc4, 0xc8, 0xcc].includes(marker);
    if (frame) {
      if (i + 9 > bytes.length) return undefined;
      return sized(view.getUint16(i + 7), view.getUint16(i + 5));
    }
    i += 2 + length;
  }
  return undefined;
}

/** Lossy (`VP8 `), lossless (`VP8L`) or extended (`VP8X`), as RFC 9649 has them. */
function webp(bytes: Uint8Array, view: DataView): Dimensions | undefined {
  if (!ascii(bytes, 0, "RIFF") || !ascii(bytes, 8, "WEBP")) return undefined;
  const byte = (i: number) => bytes[i] ?? 0;
  if (ascii(bytes, 12, "VP8 ")) {
    if (!starts(bytes, [0x9d, 0x01, 0x2a], 23) || bytes.length < 30) {
      return undefined;
    }
    return sized(
      view.getUint16(26, true) & 0x3fff,
      view.getUint16(28, true) & 0x3fff,
    );
  }
  if (ascii(bytes, 12, "VP8L")) {
    if (byte(20) !== 0x2f || bytes.length < 25) return undefined;
    const [b0, b1, b2, b3] = [byte(21), byte(22), byte(23), byte(24)];
    return sized(
      1 + (((b1 & 0x3f) << 8) | b0),
      1 + (((b3 & 0x0f) << 10) | (b2 << 2) | (b1 >> 6)),
    );
  }
  if (ascii(bytes, 12, "VP8X")) {
    if (bytes.length < 30) return undefined;
    const u24 = (i: number) =>
      byte(i) | (byte(i + 1) << 8) | (byte(i + 2) << 16);
    return sized(1 + u24(24), 1 + u24(27));
  }
  return undefined;
}

function sized(width: number, height: number): Dimensions | undefined {
  return width > 0 && height > 0 ? { width, height } : undefined;
}

function starts(bytes: Uint8Array, prefix: number[], at: number): boolean {
  return prefix.every((b, i) => bytes[at + i] === b);
}

function ascii(bytes: Uint8Array, at: number, text: string): boolean {
  return [...text].every((c, i) => bytes[at + i] === c.charCodeAt(0));
}
