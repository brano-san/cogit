import { serviceInput } from "$lib/ai-config";
import { aiKeyState, type AiConfig, type AiKeyState } from "$lib/ipc";
import { readKey, writeKey } from "$lib/settings-file";
import { masterPassword } from "$stores/master-password.svelte";
import { notices } from "$stores/notices.svelte";

const CONSENT = "aiCommitMessageConsent";

class AiSettings {
  #status = $state.raw<{ input: string; key: AiKeyState } | null>(null);
  #sequence = 0;
  #consents: Promise<Set<string>> | null = null;

  at(config: AiConfig | null): AiKeyState | null {
    return this.#status?.input === serviceInput(config) ? this.#status.key : null;
  }

  async refresh(config: AiConfig | null): Promise<void> {
    const sequence = ++this.#sequence;
    if (!config) {
      this.#status = null;
      return;
    }
    try {
      const key = await aiKeyState(config);
      if (sequence === this.#sequence) this.#status = { input: serviceInput(config), key };
    } catch (err) {
      if (sequence !== this.#sequence) return;
      this.#status = null;
      notices.report(err, "Could not read AI key status");
    }
  }

  async unlock(config: AiConfig): Promise<void> {
    try {
      if (await masterPassword.unlock()) await this.refresh(config);
    } catch (err) {
      notices.report(err, "Could not unlock the password store");
    }
  }

  #loadConsents(): Promise<Set<string>> {
    return this.#consents ??= readKey<unknown>(CONSENT).then(
      (value) => new Set(Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : []),
      (err) => {
        notices.report(err, "Could not read AI consent");
        return new Set<string>();
      },
    );
  }

  async allowed(id: string): Promise<boolean> {
    return (await this.#loadConsents()).has(id);
  }

  async remember(id: string): Promise<void> {
    const held = await this.#loadConsents();
    held.add(id);
    try {
      await writeKey(CONSENT, [...held]);
    } catch (err) {
      notices.report(err, "Could not save AI consent");
    }
  }
}

export const aiSettings = new AiSettings();
