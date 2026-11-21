export const SSH_DESKTOP_REASON = "requires a file on this computer";

export function sshHost(label: string | undefined): string | null {
  return label?.startsWith("SSH: ") ? label.slice(5) : null;
}

export function probeRoots(
  roots: readonly string[],
  labels: ReadonlyMap<string, string>,
  open: readonly string[],
  disconnected: ReadonlySet<string>,
) {
  const connected = new Set(open);
  return roots.filter((root) => {
    const label = labels.get(root);
    return label !== undefined && label !== "invalid SSH address" && !disconnected.has(label)
      && (sshHost(label) === null || connected.has(root));
  });
}

export async function retrySshHost(
  reconnect: () => Promise<void>,
  wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms)),
) {
  for (let attempt = 0; ; attempt++) {
    await wait(1000 * 2 ** attempt);
    try {
      await reconnect();
      return;
    } catch (err) {
      if (attempt === 4) throw err;
    }
  }
}
