/** Long enough for git's own walk on a repository without a commit-graph. */
export const PUBLISHED_WAIT_MS = 200;

/** `is_published` for a decision that rewrites history: until it answers, on an error
    and past `waitMs`, the commit counts as published — a needless warning beats a
    silent force-push. */
export function publishedOrAssume(check: Promise<boolean>, waitMs = PUBLISHED_WAIT_MS): Promise<boolean> {
  return new Promise((resolve) => {
    const timer = setTimeout(() => resolve(true), waitMs);
    check.then(
      (published) => {
        clearTimeout(timer);
        resolve(published);
      },
      () => {
        clearTimeout(timer);
        resolve(true);
      },
    );
  });
}
