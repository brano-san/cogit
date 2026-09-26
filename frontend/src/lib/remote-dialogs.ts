import type { LfsOp, SubmoduleOp, SubtreeOp } from "./ipc/bindings";
import type { SubtreeAction } from "./remote-menu";
import type { ConfirmRequest } from "$stores/confirm.svelte";

/** The dialogs behind Remote ▸ Submodule, Subtree and LFS, as data: one component draws
    them all, and what they ask and send is tested here rather than in markup. */

export type Field =
  | {
      kind: "text";
      name: string;
      label: string;
      placeholder?: string;
      required?: boolean;
      /** Offered while typing; free text is still accepted. */
      suggestions?: readonly string[];
      hint?: string;
    }
  | { kind: "check"; name: string; label: string; hint?: string }
  | { kind: "choice"; name: string; label: string; options: readonly string[] };

type TextField = Extract<Field, { kind: "text" }>;

export type Values = Record<string, string | boolean>;

export interface DialogSpec {
  title: string;
  /** What pressing the button will do, in a sentence or two. */
  intro: string;
  fields: readonly Field[];
  confirm: string;
  /** Styled as a warning: the action takes something away. */
  destructive: boolean;
  values: Values;
}

export interface FieldProblem {
  field: string;
  message: string;
}

export function text(values: Values, name: string): string {
  const value = values[name];
  return typeof value === "string" ? value.trim() : "";
}

function flag(values: Values, name: string): boolean {
  return values[name] === true;
}

/** The first required field left empty, reported under that field. */
export function problemOf(spec: DialogSpec, values: Values): FieldProblem | null {
  for (const field of spec.fields) {
    if (field.kind === "text" && field.required && text(values, field.name) === "") {
      return { field: field.name, message: `Enter ${field.label.toLowerCase()}` };
    }
  }
  return null;
}

type Chosen = Exclude<SubmoduleOp, "initialize" | "synchronize">;

const SUBMODULE_TEXT: Record<Chosen, { title: string; intro: string; confirm: string }> = {
  reset: {
    title: "Reset Submodule",
    intro:
      "Checks out the commit this repository records for the submodule. Commits made in the submodule stay on their branch; git refuses if local changes would be overwritten.",
    confirm: "Reset",
  },
  deactivate: {
    title: "Deactivate Submodule",
    intro:
      "Sets submodule.<name>.active to false in .git/config: recursive commands skip it, its files stay. Initialise brings it back.",
    confirm: "Deactivate",
  },
  deinit: {
    title: "Deinit Submodule",
    intro:
      "Removes the submodule's checkout and its registration in .git/config. git refuses if the submodule has local changes.",
    confirm: "Deinit",
  },
  unregister: {
    title: "Unregister Submodule",
    intro:
      "Removes the submodule from the index and from .gitmodules, ready to commit. Its files stay on disk as an untracked folder.",
    confirm: "Unregister",
  },
};

export function submoduleDialog(op: Chosen, choices: readonly string[], path: string | null): DialogSpec {
  const words = SUBMODULE_TEXT[op];
  return {
    ...words,
    fields: [{ kind: "choice", name: "path", label: "Submodule", options: choices }],
    destructive: true,
    values: { path: path ?? choices[0] ?? "" },
  };
}

export function submoduleAddDialog(): DialogSpec {
  return {
    title: "Add Submodule",
    intro: "Clones the repository into the folder and records it in .gitmodules, ready to commit.",
    fields: [
      { kind: "text", name: "url", label: "Repository URL", required: true, placeholder: "https://…/library.git" },
      { kind: "text", name: "path", label: "Path", required: true, placeholder: "libs/library" },
      { kind: "text", name: "branch", label: "Branch", placeholder: "Optional — the remote's default branch" },
    ],
    confirm: "Add",
    destructive: false,
    values: { url: "", path: "", branch: "" },
  };
}

/** The values after `changed` was edited: Add Submodule names the folder after the URL
    until the user types a path of their own. */
export function followUp(values: Values, changed: string, touched: ReadonlySet<string>): Values {
  if (changed === "url" && "path" in values && !touched.has("path")) {
    return { ...values, path: pathFromUrl(text(values, "url")) };
  }
  return values;
}

/** `library` for `https://host/team/library.git`: where a clone of it would go. */
export function pathFromUrl(url: string): string {
  const name = url.trim().replace(/[/\\]+$/, "").split(/[/\\:]/).at(-1) ?? "";
  return name.replace(/\.git$/, "");
}

export interface SubtreeContext {
  prefixes: readonly string[];
  remotes: readonly string[];
}

const PREFIX: TextField = { kind: "text", name: "prefix", label: "Folder", required: true, placeholder: "lib/vendor" };

export function subtreeDialog(action: SubtreeAction, context: SubtreeContext): DialogSpec {
  const prefix = { ...PREFIX, suggestions: context.prefixes };
  const repository = (required: boolean, hint?: string): Field => ({
    kind: "text",
    name: "repository",
    label: "Repository",
    required,
    suggestions: context.remotes,
    placeholder: "A remote's name or a URL",
    hint,
  });
  const reference = (label: string, placeholder: string): Field => ({
    kind: "text",
    name: "reference",
    label,
    required: true,
    placeholder,
  });
  const squash: Field = { kind: "check", name: "squash", label: "Squash the history into one commit" };
  const values: Values = { prefix: context.prefixes[0] ?? "", repository: "", reference: "", squash: false };

  switch (action) {
    case "add":
      return {
        title: "Add Subtree",
        intro: "Brings another repository's branch into a folder of this one, with its history, as a merge commit.",
        fields: [{ ...prefix, suggestions: [] }, repository(true), reference("Branch or tag", "main"), squash],
        confirm: "Add",
        destructive: false,
        values: { ...values, prefix: "" },
      };
    case "merge":
      return {
        title: "Merge Subtree",
        intro: "Merges newer commits of the subtree's repository into its folder.",
        fields: [
          prefix,
          repository(false, "Leave empty to merge a commit or branch this repository already has."),
          reference("Branch, tag or commit", "main"),
          squash,
        ],
        confirm: "Merge",
        destructive: false,
        values,
      };
    case "split":
      return {
        title: "Split Subtree",
        intro: "Creates a branch that holds only the folder's history, with the folder as its root.",
        fields: [
          prefix,
          { kind: "text", name: "branch", label: "New branch", required: true, placeholder: "lib-only" },
          { kind: "check", name: "rejoin", label: "Merge the split back into HEAD (--rejoin)" },
        ],
        confirm: "Split",
        destructive: false,
        values: { ...values, branch: "", rejoin: false },
      };
    case "reset":
      return {
        title: "Reset Subtree",
        intro:
          "Replaces the folder's contents with the tree of the given commit and stages the result. Refused while the folder has uncommitted changes.",
        fields: [prefix, reference("Branch, tag or commit", "lib/main")],
        confirm: "Reset",
        destructive: true,
        values,
      };
    case "push":
      return {
        title: "Push Subtree",
        intro: "Splits the folder's history and pushes it to a branch of the subtree's repository.",
        fields: [prefix, repository(true), reference("Remote branch", "main")],
        confirm: "Push",
        destructive: false,
        values,
      };
  }
}

export function subtreeRequest(action: SubtreeAction, values: Values): SubtreeOp {
  const prefix = text(values, "prefix");
  const repository = text(values, "repository");
  const reference = text(values, "reference");
  switch (action) {
    case "add":
      return { kind: "add", prefix, repository, reference, squash: flag(values, "squash") };
    case "merge":
      return { kind: "merge", prefix, repository: repository || null, reference, squash: flag(values, "squash") };
    case "split":
      return { kind: "split", prefix, branch: text(values, "branch"), rejoin: flag(values, "rejoin") };
    case "reset":
      return { kind: "reset", prefix, reference };
    case "push":
      return { kind: "push", prefix, repository, reference };
  }
}

export function lfsTrackDialog(suggestion: string): DialogSpec {
  return {
    title: "Track with Git LFS",
    intro: "Adds the pattern to .gitattributes: matching files are stored in Git LFS from the next commit on.",
    fields: [{ kind: "text", name: "pattern", label: "File pattern", required: true, placeholder: "*.psd" }],
    confirm: "Track",
    destructive: false,
    values: { pattern: suggestion },
  };
}

export function lfsTrackRequest(values: Values): LfsOp {
  return { kind: "track", pattern: text(values, "pattern") };
}

/** A plain yes-or-no, so it goes through the app's confirmation dialog. */
export const LFS_PRUNE: ConfirmRequest = {
  title: "Prune Git LFS Files",
  message:
    "Deletes local copies of LFS files that are old and already on the remote (git lfs prune). They are downloaded again when needed.",
  confirm: "Prune",
  warning: true,
};

export const LFS_DOWNLOAD = "https://git-lfs.com";

/** `again`: Check Again was pressed and git still has no `lfs` command. */
export function lfsMissingDialog(again = false): DialogSpec {
  return {
    title: "Git LFS Is Not Installed",
    intro: `${again ? "Still not found. " : ""}Git has no lfs command on this computer. Git for Windows includes it as an optional component; otherwise install it from git-lfs.com, then choose Check Again.`,
    fields: [],
    confirm: "Check Again",
    destructive: false,
    values: {},
  };
}
