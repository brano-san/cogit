interface Entry<T> {
  promise: Promise<T>;
  value?: T;
  done: boolean;
}

/** Answers kept per `scope` object: a new scope (a refreshed summary) drops them all.
    `peek` gives a settled answer synchronously, so a menu can open without an IPC wait. */
export function prefetcher<T>() {
  const scopes = new WeakMap<object, Map<string, Entry<T>>>();

  function get(scope: object, key: string, load: () => Promise<T>): Promise<T> {
    let held = scopes.get(scope);
    if (!held) scopes.set(scope, (held = new Map()));
    const found = held.get(key);
    if (found) return found.promise;
    const entry: Entry<T> = { promise: load(), done: false };
    held.set(key, entry);
    entry.promise.then(
      (value) => {
        entry.value = value;
        entry.done = true;
      },
      () => held.delete(key),
    );
    return entry.promise;
  }

  function peek(scope: object, key: string): T | undefined {
    const found = scopes.get(scope)?.get(key);
    return found?.done ? found.value : undefined;
  }

  return { get, peek };
}
