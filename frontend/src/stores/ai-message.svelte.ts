import { amendedMessage } from "$lib/commit-prefix";
import type { AiChunk, AiConfig, AiKeyState, AiMessage, AiMessageRequest, RepoId } from "$lib/ipc";

export function generateMessageBlocker(
  stagedCount: number, config: AiConfig | null, key: AiKeyState | null, active: boolean,
): string | undefined {
  if (active) return "Message generation is in progress";
  if (stagedCount === 0) return "Stage changes to generate a commit message";
  if (!config) return "Configure a provider in Preferences › AI";
  if (!key) return "Checking the AI provider settings";
  if (key.state === "locked") return "The password store is locked. Select Unlock to continue.";
  if (key.required && !key.stored) return "Add an API key in Preferences › AI";
  return undefined;
}

export interface Generation {
  config: AiConfig;
  key: AiKeyState;
  repo: RepoId;
  name: string;
  prefix: string | null;
  read: () => string;
  write: (text: string) => void;
  valid: () => boolean;
  ownsField: () => boolean;
}

interface Dependencies {
  consent: (name: string, key: AiKeyState) => Promise<{ yes: boolean; checked: boolean }>;
  replace: () => Promise<boolean>;
  allowed: (id: string) => Promise<boolean>;
  remember: (id: string) => Promise<void>;
  cancel: (id: number) => Promise<unknown>;
  generate: (repo: RepoId, request: AiMessageRequest, receive: (chunk: AiChunk) => void) => Promise<AiMessage>;
  report: (err: unknown) => void;
}

interface Attempt {
  id: number | null;
  canceled: boolean;
  restore: () => void;
}

export class AiMessageStore {
  #attempt = $state.raw<Attempt | null>(null);
  streaming = $state(false);
  shortened = $state(false);

  constructor(private readonly deps: Dependencies) {}

  get active(): boolean {
    return this.#attempt !== null;
  }

  async start(input: Generation): Promise<void> {
    if (this.active || !input.valid()) return;
    const previous = input.read();
    let received = "";
    let drawn = previous;
    const attempt: Attempt = {
      id: null,
      canceled: false,
      restore: () => {
        if (input.ownsField() && input.read() === drawn && drawn !== previous) input.write(previous);
      },
    };
    this.#attempt = attempt;
    const live = () => this.#attempt === attempt && input.valid() && input.read() === drawn;
    try {
      if (previous.trim() !== "" && previous !== input.prefix && !(await this.deps.replace())) return;
      if (!live()) return;
      const service = JSON.stringify([input.config.provider, input.key.service]);
      if (!(await this.deps.allowed(service))) {
        if (!live()) return;
        const answer = await this.deps.consent(input.name, input.key);
        if (!answer.yes || !live()) return;
        if (answer.checked) await this.deps.remember(service);
      }
      if (!live()) return;
      this.streaming = true;
      this.shortened = false;
      const result = await this.deps.generate(
        input.repo, { config: input.config, prefix: input.prefix },
        (chunk) => {
          if (chunk.kind === "started") {
            attempt.id = chunk.id;
            if (attempt.canceled) this.#stop(chunk.id);
          } else if (live()) {
            received += chunk.text;
            drawn = received;
            input.write(drawn);
          }
        },
      );
      if (this.#attempt !== attempt) return;
      if (!live()) {
        attempt.restore();
        return;
      }
      if (!result.message.trim()) throw new Error("The provider returned an empty commit message.");
      input.write(amendedMessage(result.message, input.prefix));
      this.shortened = result.shortened;
    } catch (err) {
      if (this.#attempt === attempt) {
        attempt.restore();
        if (input.valid()) this.deps.report(err);
      }
    } finally {
      if (this.#attempt === attempt) {
        this.#attempt = null;
        this.streaming = false;
      }
    }
  }

  cancel(): void {
    const attempt = this.#attempt;
    if (!attempt) return;
    attempt.canceled = true;
    this.#attempt = null;
    this.streaming = false;
    attempt.restore();
    if (attempt.id !== null) this.#stop(attempt.id);
  }

  #stop(id: number): void {
    void this.deps.cancel(id).catch(this.deps.report);
  }
}
