/** What a check found, in the words the dialog will use. */
export type UpdateOutcome =
  | { kind: "none" }
  | { kind: "available"; version: string; notes: string }
  | { kind: "failed"; reason: string };

export interface UpdateHandle {
  version: string;
  body?: string | null;
  downloadAndInstall: () => Promise<void>;
}

export interface Updates {
  check: () => Promise<UpdateHandle | null>;
  relaunch: () => Promise<void>;
  confirm: (outcome: UpdateOutcome) => boolean;
  report: (message: string) => void;
}

/** Split from the plugin so the branches can be tested without a release to point at. */
export function describe(found: UpdateHandle | null): UpdateOutcome {
  if (!found) return { kind: "none" };
  return { kind: "available", version: found.version, notes: found.body?.trim() ?? "" };
}

export function message(outcome: UpdateOutcome): string {
  switch (outcome.kind) {
    case "none":
      return "Cogit is up to date.";
    case "available":
      return outcome.notes === ""
        ? `Cogit ${outcome.version} is available. Install it and restart?`
        : `Cogit ${outcome.version} is available.\n\n${outcome.notes}\n\nInstall it and restart?`;
    case "failed":
      return `Could not check for updates.\n\n${outcome.reason}`;
  }
}

/**
 * One round of the Help ▸ Check for Updates flow. The plugin, the dialogs and the
 * relaunch all arrive as arguments, because none of them exist in a test.
 *
 * `quiet` is the start-up check: it speaks only when there is something to install.
 * Nobody wants "up to date" or "the network is down" in their face on every launch.
 */
export async function checkForUpdates(
  io: Updates,
  { quiet = false }: { quiet?: boolean } = {},
): Promise<UpdateOutcome> {
  let found: UpdateHandle | null;
  try {
    found = await io.check();
  } catch (err) {
    const outcome: UpdateOutcome = { kind: "failed", reason: String(err) };
    if (!quiet) io.report(message(outcome));
    return outcome;
  }

  const outcome = describe(found);
  if (outcome.kind === "none" || !found) {
    if (!quiet) io.report(message(outcome));
    return outcome;
  }

  if (!io.confirm(outcome)) return outcome;

  try {
    await found.downloadAndInstall();
  } catch (err) {
    const failed: UpdateOutcome = { kind: "failed", reason: String(err) };
    io.report(message(failed));
    return failed;
  }

  await io.relaunch();
  return outcome;
}
