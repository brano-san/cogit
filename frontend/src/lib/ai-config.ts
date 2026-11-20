import type { AiConfig, AiProvider } from "$lib/ipc";

export const AI_PROVIDERS = {
  anthropic: { label: "Anthropic", baseUrl: "https://api.anthropic.com", model: "claude-haiku-5-5" },
  openai: { label: "OpenAI-compatible", baseUrl: "https://api.openai.com/v1", model: "gpt-5.4-mini" },
} satisfies Record<AiProvider, { label: string; baseUrl: string; model: string }>;

export const MIN_DIFF_BYTES = 1_024;
export const MAX_DIFF_BYTES = 256_000;

export function diffBytes(value: number): number {
  return Number.isFinite(value) ? Math.min(MAX_DIFF_BYTES, Math.max(MIN_DIFF_BYTES, Math.round(value))) : 40_000;
}

export function aiConfig(provider: AiProvider): AiConfig {
  return { provider, baseUrl: "", model: "", instructions: "", maxDiffBytes: 40_000 };
}

export function knownAiConfig(value: unknown): AiConfig | null {
  if (typeof value !== "object" || value === null) return null;
  const raw = value as Record<string, unknown>;
  const provider = raw.provider;
  if (provider !== "anthropic" && provider !== "openai") return null;
  const text = (value: unknown) => typeof value === "string" ? value : "";
  const baseUrl = text(raw.baseUrl).trim();
  if (!validBaseUrl(baseUrl)) return null;
  return {
    provider,
    baseUrl,
    model: text(raw.model).trim(),
    instructions: text(raw.instructions),
    maxDiffBytes: diffBytes(typeof raw.maxDiffBytes === "number" ? raw.maxDiffBytes : NaN),
  };
}

export function validBaseUrl(base: string): boolean {
  if (base.trim() === "") return true;
  if (!/^https?:\/\/[^/]/i.test(base) || /[?#\\\s]/.test(base)) return false;
  try {
    const url = new URL(base);
    return !url.username && !url.password && !base.split("/")[2]?.includes("@");
  } catch {
    return false;
  }
}

export function serviceInput(config: AiConfig | null): string {
  return config ? JSON.stringify([config.provider, config.baseUrl]) : "";
}
