// esbuild bundles `.css` imports into main.css and scopes `.module.css`
// class names, returning the generated names.

declare module "*.module.css" {
  const classes: Readonly<Record<string, string>>;
  export default classes;
}

declare module "*.css";
