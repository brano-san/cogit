/**
 * Two panes that drive each other: setting one's `scrollTop` fires its `scroll` event, which
 * would map back onto the other and may land a pixel off, and so on. The guard remembers what
 * the view itself wrote, so the echo of that write is recognised and not mapped again.
 */
export type PaneSide = "left" | "right";

/** `scrollTop` reads back rounded to the device pixel. */
const TOLERANCE = 1;

export class ScrollGuard {
  #expected: Record<PaneSide, number | null> = { left: null, right: null };

  /** Whether `side` has to be written to reach `value`; remembers the write if so. */
  write(side: PaneSide, current: number, value: number): boolean {
    if (Math.abs(current - value) < TOLERANCE) return false;
    this.#expected[side] = value;
    return true;
  }

  /** The `scroll` event of `side` at `top`: `true` when it is the echo of our own write. */
  echo(side: PaneSide, top: number): boolean {
    const want = this.#expected[side];
    if (want === null) return false;
    this.#expected[side] = null;
    return Math.abs(top - want) <= TOLERANCE;
  }

  /** The user took over (wheel, drag): a stale expectation must not swallow their scroll. */
  reset(): void {
    this.#expected = { left: null, right: null };
  }
}
