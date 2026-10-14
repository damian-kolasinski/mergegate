import { For } from "solid-js";
import type { File } from "../api/File";
import { Mono, Row } from "../design-system";
import { Counts } from "./counts";
import styles from "./files.module.css";

/** The revision's files beside the diff, the current one selected. */
export function FileList(props: {
  files: File[];
  viewed: ReadonlySet<string>;
  current: string | undefined;
  onShow: (path: string) => void;
  onViewed: (path: string, viewed: boolean) => void;
}) {
  return (
    <nav class={styles.files} aria-label="Files">
      <ul>
        <For each={props.files}>
          {(file) => (
            <Row
              selected={file.path === props.current}
              onSelect={() => props.onShow(file.path)}
              onOpen={() => undefined}
            >
              <FileEntry
                file={file}
                viewed={props.viewed.has(file.path)}
                onViewed={(value) => props.onViewed(file.path, value)}
              />
            </Row>
          )}
        </For>
      </ul>
    </nav>
  );
}

function FileEntry(props: {
  file: File;
  viewed: boolean;
  onViewed: (viewed: boolean) => void;
}) {
  return (
    <>
      <input
        type="checkbox"
        class={styles.viewed}
        checked={props.viewed}
        aria-label={`Viewed ${props.file.path}`}
        // The row would show the file too.
        onClick={(event) => event.stopPropagation()}
        onChange={(event) => {
          props.onViewed(event.currentTarget.checked);
          // A focused field keeps keys from the shortcuts.
          event.currentTarget.blur();
        }}
      />
      <span class={styles.path} title={props.file.path}>
        <Mono>{props.file.path}</Mono>
      </span>
      <Counts
        additions={props.file.additions}
        deletions={props.file.deletions}
      />
    </>
  );
}
