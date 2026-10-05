import { setTaskbarState, type Flash, type TaskbarSignals } from "$lib/ipc";
import { signalsFor } from "$lib/taskbar";
import { network } from "$stores/network.svelte";
import { errorWindow } from "$stores/error-window.svelte";
import { notices } from "$stores/notices.svelte";
import { settings } from "$stores/settings.svelte";
import { untrack } from "svelte";

/** How something ended while nobody was looking. Any store may report one with `event`. */
export type TaskbarEvent = "success" | "error" | "warning";

/** The taskbar button's state, derived from what the page already knows: the network
    operations, the notification queue and the window's focus. The host merges and paints
    it (`set_taskbar_state`); this only reports. */
class TaskbarStore {
  focused = $state(true);
  #unviewed = $state(0);
  #flash = $state<Flash | null>(null);
  #sent = "";

  get unviewed(): number {
    return this.#unviewed;
  }

  get flash(): Flash | null {
    return this.#flash;
  }

  /** Counts and flashes only while the window is in the background: in front, the user
      sees the result themselves. An error or a warning blinks until focus; a success only
      counts — a flash reads as an alarm, and the system notification tells it. */
  event(kind: TaskbarEvent): void {
    if (this.focused) return;
    this.#unviewed += 1;
    if (kind !== "success") this.#flash = "persistent";
  }

  /** The window got focus: whatever piled up is seen. */
  viewed(): void {
    this.#unviewed = 0;
    this.#flash = null;
  }

  setFocused(focused: boolean): void {
    this.focused = focused;
    if (focused) this.viewed();
  }

  /** Call once from the main window; returns the cleanup. */
  start(): () => void {
    this.focused = document.hasFocus();
    const onFocus = () => this.setFocused(true);
    const onBlur = () => this.setFocused(false);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);

    const stop = $effect.root(() => {
      let runningBefore = false;
      let failuresAtStart = 0;
      $effect(() => {
        const running = network.running !== null;
        const failures = network.failures;
        if (!runningBefore && running) failuresAtStart = failures;
        // A failed operation reports its error itself; counting it a success too showed two.
        if (runningBefore && !running && failures === failuresAtStart) untrack(() => this.event("success"));
        runningBefore = running;
      });

      let seen = new Set<string>();
      $effect(() => {
        const queue = notices.all;
        untrack(() => {
          for (const notice of queue) {
            if (seen.has(notice.key)) continue;
            if (notice.severity === "error") this.event("error");
            else if (notice.severity === "warning") this.event("warning");
          }
        });
        seen = new Set(queue.map((notice) => notice.key));
      });

      // A failed git command or a stop on conflicts lives in the Errors window, not in the
      // notification queue: without this a failed pull showed as a success.
      let seenEntries = new Set<number>();
      $effect(() => {
        const entries = errorWindow.entries;
        untrack(() => {
          for (const entry of entries) {
            if (seenEntries.has(entry.id)) continue;
            this.event(entry.kind === "error" ? "error" : "warning");
          }
        });
        seenEntries = new Set(entries.map((entry) => entry.id));
      });

      $effect(() => {
        const queue = notices.all;
        const signals = signalsFor({
          enabled: settings.current.notificationsTaskbar,
          flashEnabled: settings.current.notificationsTaskbarFlash,
          running: network.running !== null,
          line: network.progress,
          errors: queue.filter((notice) => notice.severity === "error").length + errorWindow.errorCount,
          warnings:
            queue.filter((notice) => notice.severity === "warning").length +
            (errorWindow.warningPresent ? 1 : 0),
          unviewed: this.#unviewed,
          flash: this.#flash,
        });
        untrack(() => this.#send(signals));
      });
    });

    return () => {
      stop();
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }

  /** A flash is a one-off request; the state itself is sent only when it changed. */
  #send(signals: TaskbarSignals): void {
    const key = JSON.stringify(signals);
    if (key === this.#sent && !signals.flash) return;
    this.#sent = key;
    void setTaskbarState(signals).catch(() => {
      // No taskbar to paint (a browser, a headless run): nothing to report.
    });
    if (this.#flash !== null) this.#flash = null;
  }
}

export const taskbar = new TaskbarStore();
