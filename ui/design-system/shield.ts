import type { Tone } from "./badge";

/** A shields.io static badge, read from its URL instead of loaded. */
export type Shield = { text: string; tone: Tone; solid: boolean };

/**
 * `https://img.shields.io/badge/<label>-<message>-<color>`, the label
 * optional, as bots such as Codex mark a finding's priority with. `--` is a
 * dash and `_` a space, `__` an underscore; `?label=` and `?color=` win over
 * the path.
 */
export function shieldOf(src: string | null): Shield | undefined {
  let url: URL;
  try {
    url = new URL(src ?? "");
  } catch {
    return undefined;
  }
  const content = /^\/badge\/(.+)$/.exec(url.pathname)?.[1];
  if (url.hostname !== "img.shields.io" || !content) return undefined;
  let decoded: string;
  try {
    decoded = decodeURIComponent(content.replace(/\.svg$/, ""));
  } catch {
    return undefined;
  }
  const parts = decoded
    .replaceAll("--", "\0")
    .split("-")
    .map((part) =>
      part
        .replaceAll("\0", "-")
        .replaceAll("__", "\0")
        .replaceAll("_", " ")
        .replaceAll("\0", "_")
        .trim(),
    );
  if (parts.length < 2 || parts.length > 3) return undefined;
  const color = url.searchParams.get("color") ?? parts.at(-1) ?? "";
  const label =
    url.searchParams.get("label") ?? (parts.length === 3 ? parts[0] : "");
  const message = parts.at(-2) ?? "";
  const text = [label, message].filter(Boolean).join(" ");
  return text ? { text, ...toneOf(color) } : undefined;
}

/**
 * Shields' colours by how loud they are, as GitHub shows them: red and
 * orange solid, so a P0 or P1 outweighs a P2's yellow and a P3's green.
 */
function toneOf(color: string): { tone: Tone; solid: boolean } {
  const named = NAMED[color.toLowerCase()];
  if (named) return named;
  const hue = hueOf(color);
  if (hue === undefined) return { tone: "neutral", solid: false };
  if (hue < 15 || hue >= 330) return { tone: "error", solid: true };
  if (hue < 40) return { tone: "warning", solid: true };
  if (hue < 70) return { tone: "warning", solid: false };
  if (hue < 165) return { tone: "success", solid: false };
  if (hue < 260) return { tone: "brand", solid: false };
  return { tone: "neutral", solid: false };
}

const NAMED: Record<string, { tone: Tone; solid: boolean }> = {
  red: { tone: "error", solid: true },
  critical: { tone: "error", solid: true },
  orange: { tone: "warning", solid: true },
  important: { tone: "warning", solid: true },
  yellow: { tone: "warning", solid: false },
  yellowgreen: { tone: "success", solid: false },
  green: { tone: "success", solid: false },
  brightgreen: { tone: "success", solid: false },
  success: { tone: "success", solid: false },
  blue: { tone: "brand", solid: false },
  informational: { tone: "brand", solid: false },
};

/** A hex colour's hue in degrees; none for a grey or anything else. */
function hueOf(color: string): number | undefined {
  const hex = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(color)?.[1];
  if (!hex) return undefined;
  const full = hex.length === 3 ? [...hex].map((c) => c + c).join("") : hex;
  const [r, g, b] = [0, 2, 4].map(
    (i) => Number.parseInt(full.slice(i, i + 2), 16) / 255,
  ) as [number, number, number];
  const max = Math.max(r, g, b);
  const range = max - Math.min(r, g, b);
  if (max === 0 || range / max < 0.25) return undefined;
  const hue =
    max === r
      ? ((g - b) / range) % 6
      : max === g
        ? (b - r) / range + 2
        : (r - g) / range + 4;
  return (hue * 60 + 360) % 360;
}
