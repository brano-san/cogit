export interface RevealDeps {
  /** The commit is a row of the walk on screen. */
  inWalk: (oid: string) => Promise<boolean>;
  /** The ref has a box in Branches that is not ticked yet. */
  tickable: boolean;
  /** Ticks it and waits for the graph to be walked again with it. */
  tick: () => Promise<void>;
  reveal: (oid: string) => void;
}

/** A click on a ref's name centres the graph on its tip (F-128). A tip the walk does not
    reach is ticked first, as Reveal Commit does; one that still cannot be shown — a path
    filter hides it — is not asked for, or the request would wait for a later reload. */
export async function revealRef(oid: string, deps: RevealDeps): Promise<void> {
  if (!(await deps.inWalk(oid))) {
    if (!deps.tickable) return;
    await deps.tick();
    if (!(await deps.inWalk(oid))) return;
  }
  deps.reveal(oid);
}
