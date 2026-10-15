import { createResource, type JSX, Match, Switch } from "solid-js";
import { Dynamic } from "solid-js/web";
import type { File } from "../../api/File";
import { ExternalLink, Meta } from "../../design-system";
import { type Block, type Inline, parseMarkdown } from "./markdown";
import styles from "./markdown.module.css";

/** A Markdown file as it reads after the change. */
export function MarkdownView(props: {
  file: File;
  blobUrl: (id: string) => string;
}) {
  const [doc] = createResource(
    () => props.file.new_blob,
    async (blob) => {
      const response = await fetch(props.blobUrl(blob));
      if (!response.ok) throw new Error(`blob answered ${response.status}`);
      return parseMarkdown(await response.text());
    },
  );
  return (
    <Switch fallback={<Meta>Loading…</Meta>}>
      <Match when={doc.error as unknown}>
        <Meta>Can't load this file.</Meta>
      </Match>
      <Match when={doc()}>
        {(blocks) => (
          <div class={styles.doc}>
            <Meta>
              As it reads after the change. HTML shows as text, and images as
              their descriptions.
            </Meta>
            {blocks().map(block)}
          </div>
        )}
      </Match>
    </Switch>
  );
}

// The tree is fixed once parsed, so these build elements once, as plain
// functions rather than components.

function block(node: Block): JSX.Element {
  switch (node.kind) {
    case "heading":
      return (
        <Dynamic component={`h${node.level}`}>{node.text.map(inline)}</Dynamic>
      );
    case "paragraph":
      return <p>{node.text.map(inline)}</p>;
    case "code":
      return (
        <pre data-lang={node.lang || undefined}>
          <code>{node.text}</code>
        </pre>
      );
    case "quote":
      return <blockquote>{node.blocks.map(block)}</blockquote>;
    case "list": {
      const items = node.items.map((item) => <li>{itemBody(item)}</li>);
      return node.ordered ? (
        <ol start={node.start}>{items}</ol>
      ) : (
        <ul>{items}</ul>
      );
    }
    case "table": {
      const head = node.head.map((cell) => <th>{cell.map(inline)}</th>);
      const rows = node.rows.map((row) => {
        const cells = row.map((cell) => <td>{cell.map(inline)}</td>);
        return <tr>{cells}</tr>;
      });
      return (
        <table>
          <thead>
            <tr>{head}</tr>
          </thead>
          <tbody>{rows}</tbody>
        </table>
      );
    }
    case "rule":
      return <hr />;
  }
}

/** A list item of one paragraph reads as a line, not as a paragraph. */
function itemBody(blocks: Block[]): JSX.Element {
  const [only] = blocks;
  if (blocks.length === 1 && only?.kind === "paragraph") {
    return only.text.map(inline);
  }
  return blocks.map(block);
}

function inline(span: Inline): JSX.Element {
  switch (span.kind) {
    case "text":
      return span.text;
    case "code":
      return <code>{span.text}</code>;
    case "strong":
      return <strong>{span.text.map(inline)}</strong>;
    case "em":
      return <em>{span.text.map(inline)}</em>;
    case "del":
      return <del>{span.text.map(inline)}</del>;
    case "link":
      // Only the web opens from a preview; a relative link names a file the
      // page can't reach, and other schemes stay text.
      return /^https?:\/\//i.test(span.href) ? (
        <ExternalLink href={span.href}>{span.text.map(inline)}</ExternalLink>
      ) : (
        <span class={styles.link} title={span.href}>
          {span.text.map(inline)}
        </span>
      );
    case "image":
      return (
        <span class={styles.image}>[image: {span.alt || "untitled"}]</span>
      );
    case "break":
      return <br />;
  }
}
