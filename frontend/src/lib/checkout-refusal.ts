/** The two refusals a stash can clear. Anything else is not ours to second-guess. */
const HEADINGS = [
  "Your local changes to the following files would be overwritten by",
  "The following untracked working tree files would be overwritten by",
];

/**
 * The paths git named, or `null` when the failure was something else. An empty array means
 * git refused for this reason but named nothing — still worth offering the stash.
 */
export function blockedByLocalChanges(stderr: string): string[] | null {
  const lines = stderr.split("\n");
  const at = lines.findIndex((line) => HEADINGS.some((heading) => line.includes(heading)));
  if (at < 0) return null;

  const paths: string[] = [];
  for (const line of lines.slice(at + 1)) {
    if (!/^[\t ]+\S/.test(line)) break;
    paths.push(line.trim());
  }
  return paths;
}
