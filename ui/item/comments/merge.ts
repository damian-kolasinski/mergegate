/** `all` with what of `more` it lacks, as a refetch may have added it. */
export function merged<T extends { id: unknown }>(all: T[], more: T[]): T[] {
  const ids = new Set(all.map((each) => each.id));
  return [...all, ...more.filter((each) => !ids.has(each.id))];
}

/**
 * `latest`, with the objects of `all` that it holds unchanged, so their
 * cards, and an editor open in one, aren't rebuilt mid-typing.
 */
export function kept<T extends { id: unknown }>(all: T[], latest: T[]): T[] {
  const old = new Map(all.map((each) => [each.id, each]));
  return latest.map((each) => {
    const same = old.get(each.id);
    return same && JSON.stringify(same) === JSON.stringify(each) ? same : each;
  });
}
