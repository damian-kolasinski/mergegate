import type { File } from "../../api/File";
import { isSubmodule } from "../mode";

/** What a file's header offers to show besides its diff. */
export type Preview = "image" | "markdown";

/** The formats a browser decodes; SVG is text, and diffed as text. */
const IMAGE = /\.(png|jpe?g|gif|webp)$/i;
const MARKDOWN = /\.(md|markdown)$/i;

export function previewOf(file: File): Preview | undefined {
  if (file.interdiff === "dropped" || isSubmodule(file)) return undefined;
  if (file.content.kind === "binary" && IMAGE.test(file.path)) return "image";
  // A deleted document has nothing left to read.
  if (
    file.content.kind === "text" &&
    file.new_blob &&
    MARKDOWN.test(file.path)
  ) {
    return "markdown";
  }
  return undefined;
}

/** The label of the header's button for `preview`. */
export function previewLabel(file: File, preview: Preview): string {
  switch (preview) {
    case "image":
      return file.old_blob && file.new_blob ? "Compare images" : "Show image";
    case "markdown":
      return "Preview";
  }
}
