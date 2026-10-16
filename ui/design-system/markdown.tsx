import DOMPurify from "dompurify";
import { Marked } from "marked";
import { createMemo } from "solid-js";
import badgeStyles from "./badge.module.css";
import styles from "./markdown.module.css";
import { shieldOf } from "./shield";

/**
 * Text written as GitHub-flavoured Markdown, such as a comment from a bot or
 * an agent's reply. Rendered and then sanitized to a fixed set of tags with
 * no class, style or id, so nothing in it can run or restyle the page.
 */
export function Markdown(props: { source: string }) {
  const nodes = createMemo(() => [...render(props.source).childNodes]);
  return (
    <div
      class={styles.markdown}
      // A focused summary would take Enter and Space from the shortcuts.
      onClick={(event) => {
        if (event.target instanceof Element && event.target.closest("summary"))
          (event.target as HTMLElement).blur();
      }}
    >
      {nodes()}
    </div>
  );
}

/** What `source` reads as once rendered, on one line: for excerpts. */
export function markdownText(source: string): string {
  return (render(source).textContent ?? "").replace(/\s+/g, " ").trim();
}

// Single newlines break the line, as they do in GitHub's comments, so a reply
// typed as plain text keeps its shape.
const parser = new Marked({ gfm: true, breaks: true, async: false });

const purify = DOMPurify();
purify.setConfig({
  ALLOWED_TAGS: [
    ...["p", "br", "hr", "blockquote", "pre", "code", "kbd", "sub", "sup"],
    ...["strong", "b", "em", "i", "del", "s", "a", "img", "input"],
    ...["h1", "h2", "h3", "h4", "h5", "h6", "ul", "ol", "li"],
    ...["table", "thead", "tbody", "tr", "th", "td", "details", "summary"],
  ],
  ALLOWED_ATTR: ["href", "src", "alt", "title", "align", "start", "open"],
  ALLOW_DATA_ATTR: false,
  // Task list checkboxes, the only input Markdown makes.
  ADD_ATTR: ["type", "checked", "disabled"],
  RETURN_DOM_FRAGMENT: true,
});

function render(source: string): DocumentFragment {
  const html = parser.parse(source) as string;
  const fragment = purify.sanitize(html) as unknown as DocumentFragment;
  for (const image of fragment.querySelectorAll("img")) {
    image.replaceWith(imageLink(image));
  }
  for (const link of fragment.querySelectorAll("a")) {
    if (isOutward(link.getAttribute("href"))) {
      link.target = "_blank";
      link.rel = "noopener noreferrer";
    } else {
      // A relative link would move the hub's own router.
      link.replaceWith(...link.childNodes);
    }
  }
  for (const input of fragment.querySelectorAll("input")) {
    if (input.type === "checkbox") input.disabled = true;
    else input.remove();
  }
  return fragment;
}

/**
 * The CSP loads no image from elsewhere, which also keeps a comment's
 * tracking pixels quiet: an image is a link to it, named by its alt text,
 * and a shields.io badge a Badge in its colour.
 */
function imageLink(image: HTMLImageElement): Node {
  const label = image.alt.trim() || "Image";
  const src = image.getAttribute("src");
  const shield = shieldOf(src);
  // A linked image keeps the link it's in: anchors don't nest.
  const linked = isOutward(src) && !image.closest("a");
  if (!shield && !linked) return document.createTextNode(label);
  const element = document.createElement(linked ? "a" : "span");
  if (linked) element.setAttribute("href", src ?? "");
  if (shield) {
    element.className = `${badgeStyles.badge ?? ""} ${styles.shield ?? ""}`;
    element.dataset.tone = shield.tone;
    if (shield.solid) element.dataset.solid = "";
    element.title = label;
    element.textContent = shield.text;
  } else {
    element.className = styles.image ?? "";
    element.title = src ?? "";
    element.textContent = label;
  }
  return element;
}

function isOutward(href: string | null): boolean {
  if (!href) return false;
  try {
    return ["http:", "https:", "mailto:"].includes(new URL(href).protocol);
  } catch {
    return false;
  }
}
