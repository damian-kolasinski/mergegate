import { createMemo, For, Show } from "solid-js";
import type { WeeklyClosures } from "../api/WeeklyClosures";
import { Section } from "../design-system";
import styles from "./weekly.module.css";

const DAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const sum = (days: number[]) => days.reduce((total, count) => total + count, 0);

// Calendar dates come from the hub; UTC formatting keeps a remote browser
// from moving a date into the preceding day.
const date = (iso: string) => new Date(`${iso}T00:00:00Z`);
const dayAt = (iso: string, index: number) => {
  const value = date(iso);
  value.setUTCDate(value.getUTCDate() + index);
  return value;
};
const isoDate = (value: Date) => value.toISOString().slice(0, 10);
const format = (value: Date, options: Intl.DateTimeFormatOptions) =>
  value.toLocaleDateString(undefined, { ...options, timeZone: "UTC" });

export function WeeklyStats(props: { week: WeeklyClosures }) {
  const days = createMemo(() =>
    DAYS.map((name, index) => {
      const value = dayAt(props.week.monday, index);
      return { name, value, iso: isoDate(value) };
    }),
  );
  const totals = createMemo(() =>
    DAYS.map((_, index) =>
      props.week.repositories.reduce(
        (total, repo) => total + (repo.days[index] ?? 0),
        0,
      ),
    ),
  );
  const total = createMemo(() => sum(totals()));
  const max = createMemo(() =>
    Math.max(1, ...props.week.repositories.flatMap((repo) => repo.days)),
  );

  return (
    <div class={styles.weekly}>
      <Section title="Closed this week" count={total()}>
        <div class={styles.caption}>
          <span>
            {format(date(props.week.monday), {
              month: "short",
              day: "numeric",
            })}
            {" – "}
            {format(dayAt(props.week.monday, 6), {
              month: "short",
              day: "numeric",
              year: "numeric",
            })}
          </span>
          <span>Merged + abandoned · via mergegate</span>
        </div>
        <div class={styles.scroll}>
          <table class={styles.table}>
            <caption class={styles.srOnly}>
              Items closed per repository and day, Monday through Sunday
            </caption>
            <thead>
              <tr>
                <th scope="col">Repository</th>
                <For each={days()}>
                  {(day) => (
                    <th
                      scope="col"
                      aria-current={
                        day.iso === props.week.today ? "date" : undefined
                      }
                    >
                      {day.name}
                      <span class={styles.date}>{day.value.getUTCDate()}</span>
                    </th>
                  )}
                </For>
                <th scope="col">Total</th>
              </tr>
            </thead>
            <tbody>
              <For each={props.week.repositories}>
                {(repo) => (
                  <tr>
                    <th scope="row">{repo.project}</th>
                    <For each={days()}>
                      {(day, index) => (
                        <td>
                          <span
                            class={styles.cell}
                            data-level={Math.ceil(
                              ((repo.days[index()] ?? 0) / max()) * 3,
                            )}
                            data-future={day.iso > props.week.today}
                            title={`${repo.project} · ${format(day.value, { weekday: "long", month: "short", day: "numeric" })}: ${repo.days[index()] ?? 0} closed`}
                          >
                            {day.iso > props.week.today
                              ? "–"
                              : (repo.days[index()] ?? 0)}
                          </span>
                        </td>
                      )}
                    </For>
                    <td class={styles.total}>{sum(repo.days)}</td>
                  </tr>
                )}
              </For>
            </tbody>
            <tfoot>
              <tr>
                <th scope="row">All repositories</th>
                <For each={days()}>
                  {(day, index) => (
                    <td>
                      {day.iso > props.week.today ? "–" : totals()[index()]}
                    </td>
                  )}
                </For>
                <td class={styles.total}>{total()}</td>
              </tr>
            </tfoot>
          </table>
        </div>
        <Show when={!props.week.repositories.length}>
          <p class={styles.empty}>No items closed this week yet.</p>
        </Show>
      </Section>
    </div>
  );
}
