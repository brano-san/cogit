export interface RevealDeps {
  /** The commit is a row of the walk on screen. */
  inWalk: (oid: string) => Promise<boolean>;
  reveal: (oid: string) => void;
}

/** A click on a ref's name centres the graph on its tip (F-128) when the graph has it. It
    never ticks the ref: only its box does, and Reveal in Graph asks for that by name. A
    tip the walk does not reach is not asked for, or the request would wait for a reload. */
export async function revealRef(oid: string, deps: RevealDeps): Promise<void> {
  if (await deps.inWalk(oid)) deps.reveal(oid);
}
