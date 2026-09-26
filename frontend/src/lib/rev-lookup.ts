/** What a revision field names: `null` commit when it names none. */
export type Named<T> = { rev: string; commit: T | null } | null;

/** Looks up what `rev` names once the typing rests; the returned cleanup cancels it, and an
    answer arriving after it is dropped: it is about text typed over since. */
export function lookUpWhenSettled<T>(
  rev: string,
  resolve: (rev: string) => Promise<T | null>,
  take: (named: Named<T>) => void,
  settleMs: number,
): () => void {
  const wanted = rev.trim();
  if (wanted === "") {
    take(null);
    return () => {};
  }
  let live = true;
  const timer = setTimeout(() => {
    void resolve(wanted).then((commit) => {
      if (live) take({ rev: wanted, commit });
    });
  }, settleMs);
  return () => {
    live = false;
    clearTimeout(timer);
  };
}
