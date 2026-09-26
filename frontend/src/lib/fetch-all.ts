/** Fetch All's lanes: the backend queues each repository apart, so several fetch at once,
    and a remote that does not answer holds only its own lane (M3, "Известные ловушки"). */
export const FETCH_ALL_LANES = 4;

/** Runs `work` on every item, at most `lanes` at once. `work` reports its own failures;
    one that throws anyway does not stop the rest. */
export async function eachAtMost<T>(
  items: readonly T[],
  lanes: number,
  work: (item: T) => Promise<void>,
): Promise<void> {
  let next = 0;
  const lane = async () => {
    while (next < items.length) {
      const item = items[next] as T;
      next += 1;
      await work(item).catch(() => {});
    }
  };
  await Promise.all(Array.from({ length: Math.min(lanes, items.length) }, lane));
}
