Colors and typography live in `tokens.json`, extracted without value changes from the web's Academy palette (Figma file `KiTRYqOIJ3I30j1VjB1156`, color frame `68:43`). Layout, motion, and shadows live in `ui/design-system/tokens.css`.

Run `python3 appearance/generate.py` after editing tokens; `--check` detects stale outputs. This generates `ui/design-system/appearance.css`.

Syntax colors live in `syntax.json`, extracted without value changes from the web's Pierre palette. The same generator produces `ui/diff-view/syntax-palette.ts`. Token categories without colors retain the source foreground; emphasis, strong/headings, and links use italic, semibold, and underline.
