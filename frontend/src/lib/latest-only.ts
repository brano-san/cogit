/** `merge` keeps what a replaced value carried that the newer one must not lose. */
export function latestOnly<T>(
  send: (value: T) => Promise<void>,
  onFail: (value: T) => void,
  merge: (older: T, newer: T) => T = (_older, newer) => newer,
): (value: T) => void {
  let busy = false;
  let pending: { value: T } | null = null;
  const next = (value: T) => {
    busy = true;
    void send(value)
      .catch(() => onFail(value))
      .finally(() => {
        const queued = pending;
        pending = null;
        if (queued) next(queued.value);
        else busy = false;
      });
  };
  return (value) => {
    if (busy) pending = { value: pending ? merge(pending.value, value) : value };
    else next(value);
  };
}
