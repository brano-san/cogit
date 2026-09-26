/** Where Set Upstream… starts: the current upstream, else the branch of the same name
    (on the primary remote first), else the first remote branch. */
export function initialUpstream(
  branch: string,
  current: string | null,
  choices: readonly string[],
  primary: string | null,
): string | null {
  if (current !== null && choices.includes(current)) return current;
  const namesake = choices.filter((choice) => choice.endsWith(`/${branch}`));
  return namesake.find((choice) => choice === `${primary}/${branch}`) ?? namesake[0] ?? choices[0] ?? null;
}

export function upstreamProblem(
  choice: string | null,
  current: string | null,
  choices: readonly string[],
): string | null {
  if (choices.length === 0 || choice === null) return "There are no remote branches. Fetch first.";
  if (choice === current) return "This is already the upstream.";
  return null;
}
