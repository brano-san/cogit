const RESET_MS = 1500;

/** Clipboard write with visible outcome: a button shows `label("Copy")` and gets "Copied ✓" or
    "Copy failed" for a moment, then reverts. One instance per button. */
export class CopyFeedback {
  state = $state<"idle" | "copied" | "failed">("idle");
  #timer: ReturnType<typeof setTimeout> | undefined;

  /** Takes the text or a read of it, so a failed read shows "Copy failed" too. */
  async copy(source: string | Promise<string>) {
    try {
      const text = await source;
      if (text === "") return;
      const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
      await writeText(text);
      this.state = "copied";
    } catch {
      this.state = "failed";
    }
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => (this.state = "idle"), RESET_MS);
  }

  label(idle: string): string {
    return this.state === "copied" ? "Copied ✓" : this.state === "failed" ? "Copy failed" : idle;
  }
}
