import type { File } from "../../api/File";
import { isSubmodule } from "../mode";

/** What a file's header offers to show besides its diff. */
export type Preview = "image";

/** The formats a browser decodes; SVG is text, and diffed as text. */
const IMAGE = /\.(png|jpe?g|gif|webp)$/i;

export function previewOf(file: File): Preview | undefined {
  if (file.interdiff === "dropped") return undefined;
  if (
    file.content.kind === "binary" &&
    !isSubmodule(file) &&
    IMAGE.test(file.path)
  ) {
    return "image";
  }
  return undefined;
}

/** The label of the header's button for `preview`. */
export function previewLabel(file: File, preview: Preview): string {
  switch (preview) {
    case "image":
      return file.old_blob && file.new_blob ? "Compare images" : "Show image";
  }
}
