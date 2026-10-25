Colors and typography live in `tokens.json`, extracted without value changes from the web's Academy palette (Figma file `KiTRYqOIJ3I30j1VjB1156`, color frame `68:43`). Layout, motion, and browser-only shadows remain in `ui/design-system/tokens.css`.

Run `python3 appearance/generate.py` after editing tokens; `--check` detects stale outputs. This generates `ui/design-system/appearance.css` and `ios/Appearance/Appearance.swift`. The latter compiles privately into each feature, exposing semantic colors and Dynamic Type text styles without a shared appearance module. Font sizes, weights, line spacing, and tracking derive from the same definitions as the web. Native code uses system monospace when source rendering is added.

The app bundles DM Sans from Google Fonts (`ofl/dmsans/DMSans[opsz,wght].ttf`), licensed under the adjacent `ios/Resources/OFL.txt`.

Syntax colors live in `syntax.json`, extracted without value changes from the web's Pierre palette. The same generator produces `ui/diff-view/syntax-palette.ts` and `ios/CodeStyling/Sources/SyntaxPalette.swift`. Token categories without colors retain the source foreground; emphasis, strong/headings, and links use italic, semibold, and underline on both clients.
