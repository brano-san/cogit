/** The gray word beside a branch name: how it stands against its upstream. Level or no
    upstream says nothing. */
export function upstreamStatus(remote: string | null, ahead: number, behind: number): string | undefined {
  if (remote === null || (ahead === 0 && behind === 0)) return undefined;
  if (ahead > 0 && behind > 0) return `><${remote}`;
  return ahead > 0 ? `>${remote}` : `<${remote}`;
}
