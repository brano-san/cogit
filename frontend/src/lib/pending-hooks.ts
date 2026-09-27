import type { HookOverview } from "$lib/ipc";

const STAGES = {
  commit: ["pre-commit", "prepare-commit-msg", "commit-msg", "post-commit"],
  push: ["pre-push"],
} as const;

/** The hooks git will run for this step, in the order it runs them (D4). */
export function pendingHooks(overview: HookOverview | null, stage: keyof typeof STAGES): string[] {
  if (!overview) return [];
  const wanted: readonly string[] = STAGES[stage];
  return overview.hooks
    .filter((hook) => hook.state === "enabled" && wanted.includes(hook.name))
    .sort((a, b) => wanted.indexOf(a.name) - wanted.indexOf(b.name))
    .map((hook) => hook.name);
}

export function hooksNote(hooks: readonly string[]): string | undefined {
  return hooks.length === 0 ? undefined : `running ${hooks.join(", ")} (output in the Output panel)`;
}
