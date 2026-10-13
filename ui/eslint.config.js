// ESLint checks correctness only; Biome formats. Solid's reactivity rules are
// errors, not warnings: breaking them (destructuring props, early returns in
// components) compiles fine and silently stops updates.

import solid from "eslint-plugin-solid/configs/typescript";
import tseslint from "typescript-eslint";

const solidAsErrors = Object.fromEntries(
  Object.keys(solid.rules).map((rule) => [rule, "error"]),
);

export default tseslint.config(
  {
    ignores: [
      "node_modules/",
      "dist/",
      "api/",
      "test-results/",
      "playwright-report/",
    ],
  },
  ...tseslint.configs.strict,
  {
    files: ["**/*.{ts,tsx}"],
    ...solid,
    rules: { ...solid.rules, ...solidAsErrors },
  },
);
