import type { ResetMode } from "./ipc/bindings";

export interface ResetChoice {
  mode: ResetMode;
  label: string;
  explanation: string;
  /** Loses work, so it is confirmed before it runs. */
  destructive: boolean;
}

/** In the order of Reset Advanced…; `mixed` first because it is what plain Reset does. */
export const RESET_CHOICES: readonly ResetChoice[] = [
  {
    mode: "mixed",
    label: "Mixed",
    explanation:
      "Move the branch and reset the index. Your changes, and those of the commits left behind, stay in the working tree, unstaged.",
    destructive: false,
  },
  {
    mode: "soft",
    label: "Soft",
    explanation:
      "Move the branch only. The index and the working tree stay as they are, so the commits left behind show up as staged changes.",
    destructive: false,
  },
  {
    mode: "hard",
    label: "Hard",
    explanation:
      "Move the branch and make the index and the working tree match the commit. Uncommitted changes to tracked files are thrown away.",
    destructive: true,
  },
  {
    mode: "keep",
    label: "Keep",
    explanation:
      "Like Hard, but keeps your uncommitted changes, and refuses if one of them is in a file the reset has to change.",
    destructive: false,
  },
  {
    mode: "merge",
    label: "Merge",
    explanation:
      "Like Keep, but also keeps staged changes; meant for backing out of a merge that stopped on conflicts.",
    destructive: false,
  },
];

export function resetChoice(mode: ResetMode): ResetChoice {
  return RESET_CHOICES.find((choice) => choice.mode === mode) ?? RESET_CHOICES[0]!;
}
