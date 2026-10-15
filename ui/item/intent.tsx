import { For, Match, Show, Switch } from "solid-js";
import { CodeText } from "../design-system";
import { marked } from "../design-system/text";
import styles from "./intent.module.css";

/**
 * The agent's guide under the header: what the change is for, as it wrote
 * it. Folded to its first line, so the diff keeps the screen; the files it
 * lists are in the file list already, so they aren't repeated here.
 *
 * Only a little Markdown renders — headings, lists, paragraphs, code and
 * bold — and all of it as text, so nothing an agent sends can run or
 * restyle the page.
 */
export function Intent(props: { guide: string }) {
  const blocks = () => parse(withoutFiles(props.guide));
  const preview = () => {
    const first = blocks()[0];
    if (!first) return "";
    return plain(first.kind === "list" ? (first.items[0] ?? "") : first.text);
  };
  return (
    <details class={styles.intent}>
      <summary
        class={styles.summary}
        // A focused summary would take Enter and Space from the shortcuts.
        onClick={(event) => event.currentTarget.blur()}
      >
        <span class={styles.label}>Intent</span>
        <span class={styles.preview}>{preview()}</span>
      </summary>
      <div class={styles.text} role="region" aria-label="Intent">
        <For each={blocks()}>
          {(block) => (
            <Switch>
              <Match when={block.kind === "heading" && block}>
                {(heading) => (
                  <p class={styles.heading}>
                    <Inline text={heading().text} />
                  </p>
                )}
              </Match>
              <Match when={block.kind === "list" && block}>
                {(list) => (
                  <ul class={styles.list}>
                    <For each={list().items}>
                      {(item) => (
                        <li>
                          <Inline text={item} />
                        </li>
                      )}
                    </For>
                  </ul>
                )}
              </Match>
              <Match when={block.kind === "code" && block}>
                {(code) => <pre class={styles.code}>{code().text}</pre>}
              </Match>
              <Match when={block.kind === "paragraph" && block}>
                {(paragraph) => (
                  <p>
                    <Inline text={paragraph().text} />
                  </p>
                )}
              </Match>
            </Switch>
          )}
        </For>
      </div>
    </details>
  );
}

type Block =
  | { kind: "heading" | "paragraph" | "code"; text: string }
  | { kind: "list"; items: string[] };

/** `**bold**` around `code`, each part as text. */
function Inline(props: { text: string }) {
  return (
    <For each={marked(props.text, /\*\*(.+?)\*\*/)}>
      {(part) => (
        <Show when={part.marked} fallback={<CodeText text={part.text} />}>
          <strong>
            <CodeText text={part.text} />
          </strong>
        </Show>
      )}
    </For>
  );
}

const HEADING = /^(#{1,6})\s+(.*)$/;
const BULLET = /^\s*(?:[-*+]|\d+[.)])\s+(.*)$/;

/**
 * The guide without its `## Files` section, down to the next heading as
 * high: the hub reads it to order the files, and the file list shows it.
 */
function withoutFiles(guide: string): string {
  const kept: string[] = [];
  let skipping: number | undefined;
  for (const line of guide.split("\n")) {
    const heading = HEADING.exec(line);
    if (heading) {
      const level = heading[1]?.length ?? 0;
      if (skipping !== undefined && level <= skipping) skipping = undefined;
      if (heading[2]?.trim().toLowerCase() === "files") {
        skipping = level;
        continue;
      }
    }
    if (skipping === undefined) kept.push(line);
  }
  return kept.join("\n");
}

function parse(text: string): Block[] {
  const blocks: Block[] = [];
  let paragraph: string[] = [];
  let list: string[] | undefined;
  let code: string[] | undefined;
  const flush = () => {
    if (paragraph.length) {
      blocks.push({ kind: "paragraph", text: paragraph.join(" ") });
    }
    if (list) blocks.push({ kind: "list", items: list });
    paragraph = [];
    list = undefined;
  };
  for (const line of text.split("\n")) {
    if (code) {
      if (line.trimStart().startsWith("```")) {
        blocks.push({ kind: "code", text: code.join("\n") });
        code = undefined;
      } else {
        code.push(line);
      }
      continue;
    }
    if (line.trimStart().startsWith("```")) {
      flush();
      code = [];
      continue;
    }
    const heading = HEADING.exec(line);
    const bullet = BULLET.exec(line);
    if (!line.trim()) {
      flush();
    } else if (heading) {
      flush();
      blocks.push({ kind: "heading", text: heading[2] ?? "" });
    } else if (bullet) {
      if (paragraph.length) flush();
      list ??= [];
      list.push(bullet[1] ?? "");
    } else if (list && /^\s+/.test(line)) {
      // A wrapped bullet goes on with the one above.
      list[list.length - 1] += ` ${line.trim()}`;
    } else {
      if (list) flush();
      paragraph.push(line.trim());
    }
  }
  if (code) blocks.push({ kind: "code", text: code.join("\n") });
  flush();
  return blocks;
}

/** A line of the guide with its Markdown marks dropped, for the preview. */
function plain(text: string): string {
  return text.replace(/\*\*(.+?)\*\*/g, "$1").replace(/`([^`]+)`/g, "$1");
}
