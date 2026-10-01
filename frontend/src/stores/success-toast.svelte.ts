/** How long the toast stays fully visible, then how long it takes to fade (CSS matches). */
export const TOAST_HOLD_MS = 1500;
export const TOAST_FADE_MS = 300;

/** A transient "it worked" line for a finished operation. A newer one replaces the one on
    screen and restarts the clock, so they never pile up. Failures are not shown here: they
    stay in the notification queue. */
class SuccessToast {
  text = $state<string | null>(null);
  leaving = $state(false);
  /** Changes on every `show`, so a replaced toast restarts its fade animation. */
  id = $state(0);
  #fade: ReturnType<typeof setTimeout> | undefined;
  #end: ReturnType<typeof setTimeout> | undefined;

  show(text: string): void {
    this.clear();
    this.id += 1;
    this.text = text;
    this.#fade = setTimeout(() => (this.leaving = true), TOAST_HOLD_MS);
    this.#end = setTimeout(() => this.clear(), TOAST_HOLD_MS + TOAST_FADE_MS);
  }

  clear(): void {
    clearTimeout(this.#fade);
    clearTimeout(this.#end);
    this.text = null;
    this.leaving = false;
  }
}

export const successToast = new SuccessToast();
