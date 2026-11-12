/** What a check found, in the words the dialog will use. */
export type UpdateOutcome =
  | { kind: "none" }
  | { kind: "available"; version: string; notes: string }
  | { kind: "failed"; reason: string };

export type DownloadEvent =
  | { event: "Started"; data: { contentLength?: number } }
  | { event: "Progress"; data: { chunkLength: number } }
  | { event: "Finished" };

export interface UpdateHandle {
  version: string;
  body?: string | null;
  downloadAndInstall: (onEvent?: (event: DownloadEvent) => void) => Promise<void>;
}

export interface Updates {
  check: () => Promise<UpdateHandle | null>;
  relaunch: () => Promise<void>;
  /** The app's own dialog answers later, so the answer may be a promise. */
  confirm: (outcome: UpdateOutcome) => boolean | Promise<boolean>;
  report: (outcome: UpdateOutcome) => void | Promise<void>;
  /** Unsaved work and running operations: false keeps the app as it is (the Exit question). */
  mayInstall: () => Promise<boolean>;
  progress?: (label: string | null) => void;
}

/** Split from the plugin so the branches can be tested without a release to point at. */
export function describe(found: UpdateHandle | null): UpdateOutcome {
  if (!found) return { kind: "none" };
  return { kind: "available", version: found.version, notes: found.body?.trim() ?? "" };
}

/** A portable copy never replaces itself: the user swaps the folder, the data stays. */
export const PORTABLE_UPDATE_NOTE =
  "This is the portable build of Cogit and it never updates itself. Download the new version from the releases page and replace this one; your settings stay in the Cogit-data folder beside it.";

/** A build without the updater plugin (macOS): nothing to check, only a place to download. */
export const NO_SELF_UPDATE_NOTE =
  "This build of Cogit does not update itself. Download the new version from the releases page and install it over this one.";

/** Why this build cannot update itself, or `null` when it can. */
export function noSelfUpdateNote(info: { portableDir: string | null; selfUpdate: boolean } | null): string | null {
  if (info?.portableDir) return PORTABLE_UPDATE_NOTE;
  return info && !info.selfUpdate ? NO_SELF_UPDATE_NOTE : null;
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
  { quiet = false }: { quiet?: boolean | (() => boolean) } = {},
): Promise<UpdateOutcome> {
  // Read when needed: Check for Updates pressed during the start-up check makes it speak.
  const silent = () => (typeof quiet === "function" ? quiet() : quiet);
  let found: UpdateHandle | null;
  try {
    found = await io.check();
  } catch (err) {
    const outcome: UpdateOutcome = { kind: "failed", reason: String(err) };
    if (!silent()) io.report(outcome);
    return outcome;
  }

  const outcome = describe(found);
  if (outcome.kind === "none" || !found) {
    if (!silent()) io.report(outcome);
    return outcome;
  }

  if (!(await io.confirm(outcome))) return outcome;
  // Installing ends the app like Exit does, so it asks what Exit asks first.
  if (!(await io.mayInstall())) return outcome;

  const name = `Cogit ${found.version}`;
  let total = 0;
  let received = 0;
  io.progress?.(`Downloading ${name}…`);
  try {
    await found.downloadAndInstall((event) => {
      if (event.event === "Started") total = event.data.contentLength ?? 0;
      if (event.event === "Progress") received += event.data.chunkLength;
      if (event.event === "Finished") {
        io.progress?.(`Installing ${name}…`);
        return;
      }
      const percent = total > 0 ? ` ${Math.min(Math.floor((received * 100) / total), 100)}%` : "";
      io.progress?.(`Downloading ${name}…${percent}`);
    });
  } catch (err) {
    io.progress?.(null);
    const failed: UpdateOutcome = { kind: "failed", reason: String(err) };
    io.report(failed);
    return failed;
  }

  await io.relaunch();
  return outcome;
}

/**
 * At most one check at a time. The start-up check and Help ▸ Check for Updates ran side by
 * side: one was downloading while the other asked to install the same version again. A
 * second call joins the running one; a manual call during the start-up check makes it speak.
 */
export function oneCheckAtATime(
  run: (quiet: () => boolean) => Promise<void>,
): (quiet?: boolean) => Promise<void> {
  let running: Promise<void> | null = null;
  let loud = false;
  return (quiet = false) => {
    if (!quiet) loud = true;
    if (running) return running;
    loud = !quiet;
    running = run(() => !loud).finally(() => (running = null));
    return running;
  };
}
