import { DEFAULT_SETTINGS, type GraphColumn, type Settings } from "./settings";

export type FieldKey = keyof Settings | "keymap" | "suppressions" | "toolbar";

/** Rows that show something other than a setting: the keymap, the hidden-dialogs list,
    the toolbar layout (kept with the toolbar's own choices). */
export function isSetting(key: FieldKey): key is keyof Settings {
  return key !== "keymap" && key !== "suppressions" && key !== "toolbar";
}

export interface Field {
  key: FieldKey;
  label: string;
  hint?: string;
  /** Words the user might search for that the label does not contain. */
  keywords?: string[];
  /** The option this one only makes sense under. Indenting a row in the markup is a
      claim about behaviour, and this is what makes the claim true (R-117). */
  dependsOn?: keyof Settings;
  /** Same, for an option about a graph column: off while that column is hidden. */
  dependsOnColumn?: GraphColumn;
}

export interface Group {
  title: string;
  fields: Field[];
}

export interface Category {
  id: string;
  title: string;
  parent?: string;
  groups: Group[];
  /** Shown in the footer while this category is open. */
  note?: string;
}

/** Headings carry no fields; the tree needs them so a child has a path to show under. */
const heading = (id: string, title: string): Category => ({ id, title, groups: [] });

export const CATEGORIES: Category[] = [
  heading("commands", "Commands"),
  {
    id: "git",
    title: "General",
    parent: "commands",
    note: "*) Changing the Git executable takes effect after a restart.",
    groups: [
      {
        // Where the Exit dialog's "Don't show again" hint sends the user (R-169).
        title: "Exiting",
        fields: [
          {
            key: "confirmExit",
            label: "Confirm before exiting",
            hint: "Cogit still asks while an operation is running or waiting, whatever this says.",
            keywords: ["exit", "quit", "close", "confirm", "ask", "dont show"],
          },
        ],
      },
      {
        title: "Pull",
        fields: [
          {
            key: "pullMode",
            label: "When a pull cannot fast-forward",
            keywords: ["merge", "rebase", "fetch"],
          },
          {
            key: "backgroundFetchMinutes",
            label: "Check the remotes in the background",
            hint: "Keeps the pull arrows in Repositories current by asking each remote what it has; nothing is fetched. Never asks for credentials; a remote that cannot be asked shows ? instead.",
            keywords: ["fetch", "background", "interval", "remote", "pull", "arrow"],
          },
        ],
      },
      {
        title: "Git",
        fields: [
          {
            key: "gitPath",
            label: "Git executable *",
            hint: "Leave as `git` to use the one on PATH.",
            keywords: ["binary", "path", "executable"],
          },
        ],
      },
    ],
  },
  {
    id: "auth",
    title: "Authentication",
    parent: "commands",
    groups: [{ title: "Hosting tokens", fields: [] }],
  },
  heading("ui", "User Interface"),
  {
    id: "theme",
    title: "Theme & Colours",
    parent: "ui",
    groups: [{ title: "Appearance", fields: [{ key: "theme", label: "Theme", keywords: ["dark", "light", "grey", "gray"] }] }],
  },
  {
    id: "graph",
    title: "Graph & History",
    parent: "ui",
    groups: [
      {
        title: "Columns",
        fields: [
          {
            key: "graphColumns",
            label: "Columns",
            hint: "Drag a row by its handle, or focus the handle and press Alt+↑ / Alt+↓, to change the order.",
            keywords: ["author", "avatar", "time", "date", "hash", "sha", "order", "hide"],
          },
          {
            key: "graphTimeFormat",
            label: "Time",
            keywords: ["relative", "date", "time", "clock"],
            dependsOnColumn: "time",
          },
        ],
      },
      {
        title: "Rows",
        fields: [
          { key: "graphDensity", label: "Row height", keywords: ["density", "compact", "comfortable"] },
          {
            key: "graphStripes",
            label: "Alternate row background",
            keywords: ["stripes", "zebra", "banded"],
          },
        ],
      },
      {
        title: "Graph",
        fields: [
          { key: "laneWidth", label: "Lane width", keywords: ["column", "spacing"] },
          {
            key: "coloredLanes",
            label: "Coloured branch lines",
            hint: "Off: the main line is light and every other line one grey.",
            keywords: ["colour", "color", "lanes", "branches", "rainbow"],
          },
          {
            key: "graphHighlightChecked",
            label: "Colour the branches ticked in Branches",
            keywords: ["highlight", "checked", "ticked", "colour", "color"],
          },
          {
            key: "graphLongLinkRows",
            label: "Cut links longer than",
            hint: "A longer link is drawn as two arrows, at the commit and at its parent. 0 draws every link whole.",
            keywords: ["long", "link", "stub", "arrow", "threshold"],
          },
        ],
      },
      {
        title: "Modes",
        fields: [
          {
            key: "graphFirstParent",
            label: "First parents only",
            hint: "One line of history: what was merged in is not listed.",
            keywords: ["first-parent", "linear", "mainline"],
          },
          {
            key: "graphBranchOfCommit",
            label: "Highlight the branch of the clicked commit",
            keywords: ["branch", "highlight", "click"],
          },
          {
            key: "graphAncestry",
            label: "Dim everything but the ancestors and descendants of the selection",
            keywords: ["ancestors", "descendants", "ancestry", "dim"],
          },
          {
            key: "graphCollapseMerged",
            label: "Collapse merged branches",
            hint: "A merged branch folds into one row at its merge commit.",
            keywords: ["collapse", "fold", "merged"],
          },
        ],
      },
      {
        title: "Authors",
        fields: [
          {
            key: "avatars",
            label: "Author avatars",
            hint: "Gravatar is asked for an MD5 of the address, never the address itself.",
            keywords: ["gravatar", "picture", "face", "network"],
          },
        ],
      },
      {
        title: "History",
        fields: [
          {
            key: "dateFormat",
            label: "Dates in Blame and commit details",
            hint: "The graph's own time column is set under Columns above.",
            keywords: ["relative", "weekday", "yesterday", "time"],
          },
        ],
      },
    ],
  },
  {
    id: "keymap",
    title: "Keyboard",
    parent: "ui",
    note: "A key taken by another command is marked; the menu still obeys the last one saved.",
    groups: [
      {
        title: "Shortcuts",
        fields: [
          { key: "keymap", label: "Shortcuts", keywords: ["keyboard", "shortcut", "accelerator", "binding"] },
        ],
      },
    ],
  },
  {
    id: "toolbar",
    title: "Toolbar",
    parent: "ui",
    note: "Changes show on the toolbar at once. Undo takes them back one at a time.",
    groups: [
      {
        title: "Buttons",
        fields: [
          {
            key: "toolbar",
            label: "Toolbar buttons",
            keywords: ["toolbar", "buttons", "customise", "customize", "configure", "separator"],
          },
        ],
      },
    ],
  },
  {
    id: "behaviour",
    title: "Behaviour",
    parent: "ui",
    groups: [
      {
        title: "Don't show again",
        fields: [
          {
            key: "suppressions",
            label: "Choices made with \"Don't show again\" or \"Ignore for this repository\"",
            keywords: ["reset", "dont show", "ignore", "warning", "suppressed", "hidden"],
          },
        ],
      },
    ],
  },
  heading("diffmerge", "Diff & Merge"),
  {
    id: "diff",
    title: "Diff View",
    parent: "diffmerge",
    groups: [
      {
        title: "Algorithm",
        fields: [
          { key: "algorithm", label: "Diff algorithm", keywords: ["histogram", "myers"] },
          { key: "ignoreWhitespace", label: "Whitespace", keywords: ["blank", "trailing"] },
          { key: "contextLines", label: "Context lines", keywords: ["surrounding"] },
        ],
      },
      {
        title: "Side by side",
        fields: [
          {
            key: "diffSplit",
            label: "Width of the old side",
            hint: "The divider between the two columns sets it too.",
            keywords: ["split", "columns", "divider", "left", "right"],
          },
        ],
      },
    ],
  },
  {
    id: "moves",
    title: "Move Detection",
    parent: "diffmerge",
    groups: [
      {
        title: "Within a diff",
        fields: [
          { key: "detectMoves", label: "Mark moved blocks instead of delete plus insert" },
          {
            key: "wordDiff",
            label: "Highlight changed words inside a line",
            keywords: ["intraline"],
            dependsOn: "detectMoves",
          },
        ],
      },
    ],
  },
  heading("tools", "Tools & Integrations"),
  {
    id: "cli",
    title: "CLI & Terminal",
    parent: "tools",
    note: "*) The log level takes effect after a restart.",
    groups: [
      {
        title: "Terminal",
        fields: [
          {
            key: "terminal",
            label: "Open in Terminal uses",
            keywords: ["shell", "powershell", "cmd", "bash", "console"],
          },
        ],
      },
      {
        title: "Logging",
        fields: [
          {
            key: "logLevel",
            label: "Log level *",
            hint: "Profiling always writes at info, whatever this says.",
            keywords: ["debug", "trace", "diagnostics", "profiling"],
          },
        ],
      },
    ],
  },
  heading("advanced", "Advanced"),
  {
    id: "updates",
    title: "Updates",
    parent: "advanced",
    groups: [
      {
        title: "Automatic checks",
        fields: [
          {
            key: "autoUpdate",
            label: "Check for updates when Cogit starts",
            hint: "Off by default: nothing is asked of the network until this is ticked.",
            keywords: ["update", "upgrade", "version", "network", "release", "download"],
          },
        ],
      },
    ],
  },
];

function haystack(category: Category): string {
  const fields = category.groups.flatMap((group) => [
    group.title,
    ...group.fields.flatMap((field) => [field.label, field.hint ?? "", ...(field.keywords ?? [])]),
  ]);
  return [category.title, ...fields].join(" ").toLowerCase();
}

/** Ids of every category to show, parents of matching children included. */
export function matchingCategories(query: string): string[] {
  const needle = query.trim().toLowerCase();
  if (needle === "") return CATEGORIES.map((category) => category.id);

  const hit = new Set<string>();
  for (const category of CATEGORIES) {
    if (!haystack(category).includes(needle)) continue;
    hit.add(category.id);
    if (category.parent) hit.add(category.parent);
  }
  return CATEGORIES.filter((category) => hit.has(category.id)).map((category) => category.id);
}

/** Where the search should land: a page with fields on it, not a heading. */
export function firstMatch(query: string): string | null {
  const found = matchingCategories(query);
  const leaf = CATEGORIES.find(
    (category) => found.includes(category.id) && category.groups.length > 0,
  );
  return leaf?.id ?? null;
}

/** Which settings the draft moved. Key by key, not by serialising both sides: the
    dialog asks on every keystroke, and JSON also calls two keymaps different when they
    hold the same bindings in another order. */
export function changedKeys(draft: Settings, saved: Settings): (keyof Settings)[] {
  return (Object.keys(DEFAULT_SETTINGS) as (keyof Settings)[]).filter(
    (key) => !sameValue(draft[key], saved[key]),
  );
}

/** A list setting is a new array after every edit; equal items are an unchanged value. */
function sameValue(a: unknown, b: unknown): boolean {
  if (Array.isArray(a) && Array.isArray(b)) {
    return a.length === b.length && a.every((item, index) => item === b[index]);
  }
  return a === b;
}

export function sameKeymap(
  draft: Readonly<Record<string, string>>,
  saved: Readonly<Record<string, string>>,
): boolean {
  const keys = Object.keys(draft);
  if (keys.length !== Object.keys(saved).length) return false;
  return keys.every((key) => draft[key] === saved[key]);
}

/** Restore Defaults acts on the page in front of the user, not on everything. */
export function restoreCategory(draft: Settings, id: string): Settings {
  const category = CATEGORIES.find((entry) => entry.id === id);
  if (!category) return draft;

  const restored = { ...draft };
  for (const group of category.groups) {
    for (const field of group.fields) {
      if (!isSetting(field.key)) continue;
      (restored[field.key] as Settings[keyof Settings]) = DEFAULT_SETTINGS[field.key];
    }
  }
  return restored;
}

/** The keymap half of Restore Defaults: the page that edits shortcuts drops every override. */
export function restoreKeys(
  keymap: Readonly<Record<string, string>>,
  id: string,
): Record<string, string> {
  const category = CATEGORIES.find((entry) => entry.id === id);
  const holds = category?.groups.some((group) => group.fields.some((field) => field.key === "keymap"));
  return holds ? {} : { ...keymap };
}

/** Is this field switched off because the option above it is? */
export function disabledBy(current: Settings, parent: keyof Settings | undefined): boolean {
  if (parent === undefined) return false;
  const value = current[parent];
  return typeof value === "boolean" && !value;
}

export function fieldDisabled(current: Settings, field: Field): boolean {
  if (disabledBy(current, field.dependsOn)) return true;
  return field.dependsOnColumn !== undefined && !current.graphColumns.includes(field.dependsOnColumn);
}
