export function latestOnly<T>(send: (value: T) => Promise<void>, onFail: (value: T) => void): (value: T) => void {
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
    if (busy) pending = { value };
    else next(value);
  };
}
