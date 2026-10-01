import type { Branch, Tag } from "$lib/ipc";

export interface RevisionOption {
  /** What goes into the field and to git: a ref name or a full id. */
  value: string;
  label: string;
  /** The field shows this instead of `value` while it is not being edited. */
  display?: string;
  /** Group header; empty for the entries that stand on their own. */
  group: string;
  hint?: string;
  disabled?: boolean;
  reason?: string;
}

export type RevisionRow =
  | { kind: "group"; title: string }
  | { kind: "option"; option: RevisionOption; index: number };

const byName = (a: Branch | Tag, b: Branch | Tag) => a.name.localeCompare(b.name);

/** Rows for `RevisionCombobox`. `special` adds HEAD and the selected commit; `held` maps a
    local branch to the worktree that has it; `localTwins` disables a remote branch whose
    local namesake exists (a tracking branch could not take that name). */
export function revisionOptions(input: {
  branches: readonly Branch[];
  tags: readonly Tag[];
  selectedCommit?: string | null;
  special?: boolean;
  held?: ReadonlyMap<string, string>;
  localTwins?: boolean;
}): RevisionOption[] {
  const { branches, tags, selectedCommit = null, special = true, held, localTwins = false } = input;
  const options: RevisionOption[] = [];
  if (special) {
    options.push({ value: "HEAD", label: "HEAD", group: "" });
    if (selectedCommit) {
      options.push({
        value: selectedCommit,
        label: "Selected commit",
        display: "Selected commit",
        hint: selectedCommit.slice(0, 8),
        group: "",
      });
    }
  }
  const locals = branches.filter((branch) => branch.kind === "local").sort(byName);
  const names = new Set(locals.map((branch) => branch.name));
  for (const branch of locals) {
    const where = held?.get(branch.name);
    options.push({
      value: branch.name,
      label: branch.name,
      group: "Local branches",
      disabled: where !== undefined,
      reason: where === undefined ? undefined : `Checked out in ${where}`,
    });
  }
  for (const branch of branches.filter((b) => b.kind === "remote" && !b.name.endsWith("/HEAD")).sort(byName)) {
    const twin = branch.name.slice(branch.name.indexOf("/") + 1);
    const taken = localTwins && names.has(twin);
    options.push({
      value: branch.name,
      label: branch.name,
      group: "Remote branches",
      disabled: taken,
      reason: taken ? `Local branch ${twin} exists; pick it instead` : undefined,
    });
  }
  for (const entry of tags.filter((t) => t.pointsToCommit).sort(byName)) {
    options.push({ value: entry.name, label: entry.name, group: "Tags" });
  }
  return options;
}

/** At most `limit` matches; `options` is the same list without the headers, in row order,
    for the keyboard. `hidden` counts the matches that did not fit. */
export function filterRevisions(
  options: readonly RevisionOption[],
  query: string,
  limit = 200,
): { rows: RevisionRow[]; options: RevisionOption[]; hidden: number } {
  const needle = query.trim().toLowerCase();
  const matches = options.filter(
    (option) => needle === "" || option.label.toLowerCase().includes(needle) || option.value.toLowerCase().includes(needle),
  );
  const shown = matches.slice(0, limit);
  const rows: RevisionRow[] = [];
  let group: string | null = null;
  shown.forEach((option, index) => {
    if (option.group !== "" && option.group !== group) rows.push({ kind: "group", title: option.group });
    group = option.group;
    rows.push({ kind: "option", option, index });
  });
  return { rows, options: shown, hidden: matches.length - shown.length };
}

/** The next enabled entry in `step` direction; stays where it is at the ends. */
export function stepOption(options: readonly { disabled?: boolean }[], from: number, step: 1 | -1): number {
  for (let at = from + step; at >= 0 && at < options.length; at += step) {
    if (!options[at]!.disabled) return at;
  }
  return from < 0 && step === -1 ? from : Math.min(Math.max(from, -1), options.length - 1);
}
