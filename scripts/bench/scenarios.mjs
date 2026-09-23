// The scenarios, in the order a cold run meets them. The clock runs only inside `measure`;
// `prep` and `reset` put the state back so every run sees the same thing (page.mjs has the
// definition of done).
//
// A target is { sel, text?, exact?, index?, child?, section? }: the element matching `sel`
// (whose text contains `text`, or the `index`-th), inside a Files pane headed `section`,
// and optionally its descendant `child`.

const GRAPH_ROWS = `document.querySelectorAll('[role="listitem"]').length > 0`;
const REF_ROWS = `document.querySelectorAll('[role="treeitem"]').length > 0`;
const ALL = ["small", "medium", "large", "submodules", "dirty"];

const COMMIT_ROW = (index) => ({ sel: `[role="listitem"]`, index });
const REPO_ROW = (name) => ({ sel: `.wrapper .row`, text: name, exact: ".name" });
const REF_GROUP = (label, child) => ({ sel: `[role="treeitem"].group`, text: label, child });
// The name, not the row centre: the row's Stage and Discard buttons sit there on hover.
const FILE_ROW = (name, section) => ({ sel: `.pane button.row`, text: name, section, child: ".name" });
const FILTER_FILES = `input[placeholder="File Filter"]`;
const FILTER_REFS = `input[aria-label="Filter references"]`;
const WORKTREE_ROW = (index) => ({ sel: `[aria-label="Worktrees"] [role="option"]`, index });
const HEAVY = { quiet: 400, timeout: 180_000 };

/** The Working Tree row exists only while the list is scrolled to its top. */
async function workingTree(ctx) {
  await ctx.prep.run(`(() => { document.querySelector('.scroll[aria-label="Commits"]').scrollTop = 0; })()`);
  await ctx.prep.click({ sel: "button.row.header" });
}

async function setGroup(ctx, label, open, kind = "group") {
  const row = { sel: `[role="treeitem"].${kind}`, text: label };
  const expanded = await ctx.attr(row, "aria-expanded");
  if (expanded !== String(open)) await ctx.prep.click({ ...row, child: "button.disclosure" });
}

/** A branch leaf: `feature/003` sits under the folder `feature`, labelled `003`. */
const BRANCH_BOX = { sel: `[role="treeitem"].local`, text: "003", exact: ".label", child: "input.box" };

export const SCENARIOS = [
  {
    id: "repo.open",
    group: "Репозиторий",
    title: "открытие",
    sets: ALL,
    async prep(ctx) {
      if (await ctx.exists(REPO_ROW(ctx.set))) await ctx.prep.click({ ...REPO_ROW(ctx.set), child: `.act[title^="Close"]` });
    },
    async measure(ctx) {
      const r = await ctx.measure.run(ctx.dropScript(ctx.repo), { quiet: 300, probes: { graph: GRAPH_ROWS, refs: REF_ROWS } });
      return {
        "repo.open": r,
        "graph.first-screen": { total: r.marks.graph ?? r.total },
        "graph.full-layout": { total: r.byCommand.load_commits?.lastEnd ?? r.total, ipc: r.byCommand.load_commits?.ms ?? 0 },
        "branches.load": { total: r.marks.refs ?? r.total },
      };
    },
  },
  {
    id: "commit.select",
    group: "Коммит",
    title: "выбор коммита",
    sets: ALL,
    measure: (ctx) => ctx.measure.click(COMMIT_ROW(3 + (ctx.iteration % 10))),
  },
  {
    id: "commit.next",
    group: "Коммит",
    title: "соседний коммит (↓)",
    sets: ALL,
    prep: (ctx) => ctx.prep.click(COMMIT_ROW(2 + (ctx.iteration % 10))),
    measure: (ctx) => ctx.measure.key("ArrowDown"),
  },
  {
    id: "graph.scroll",
    group: "Граф",
    title: "прокрутка на 1 000 строк",
    sets: ["medium", "large"],
    measure: (ctx) =>
      ctx.measure.run(`(() => { document.querySelector('.scroll[aria-label="Commits"]').scrollTop = ${((ctx.iteration % 4) + 1) * 24000}; })()`),
    reset: (ctx) => ctx.prep.run(`(() => { document.querySelector('.scroll[aria-label="Commits"]').scrollTop = 0; })()`),
  },
  {
    id: "branches.expand",
    group: "Branches",
    title: "раскрытие группы",
    sets: ["medium", "large"],
    prep: (ctx) => setGroup(ctx, "Local", false),
    measure: (ctx) => ctx.measure.click(REF_GROUP("Local", "button.disclosure")),
  },
  {
    id: "branches.filter",
    group: "Branches",
    title: "фильтр",
    sets: ["medium", "large"],
    measure: (ctx) => ctx.measure.input(FILTER_REFS, "feature/01"),
    reset: (ctx) => ctx.prep.input(FILTER_REFS, ""),
  },
  {
    id: "graph.tick-branch",
    group: "Граф",
    title: "отметка ветки в Branches",
    sets: ["medium", "large"],
    async prep(ctx) {
      await setGroup(ctx, "Local", true);
      await setGroup(ctx, "feature", true, "folder");
    },
    measure: (ctx) => ctx.measure.click(BRANCH_BOX, HEAVY),
    reset: (ctx) => ctx.prep.click(BRANCH_BOX, HEAVY),
  },
  {
    id: "graph.tick-tags",
    group: "Граф",
    title: "отметка группы тегов",
    sets: ["medium", "large"],
    measure: (ctx) => ctx.measure.click(REF_GROUP("Tags", "input.box"), HEAVY),
    reset: (ctx) => ctx.prep.click(REF_GROUP("Tags", "input.box"), HEAVY),
  },
  {
    id: "files.status",
    group: "Files",
    title: "статус рабочей копии (Refresh)",
    sets: ALL,
    prep: workingTree,
    measure: (ctx) => ctx.measure.menu("refresh", HEAVY),
  },
  {
    id: "files.filter",
    group: "Files",
    title: "фильтр по имени",
    sets: ["dirty", "medium"],
    prep: workingTree,
    measure: (ctx) => ctx.measure.input(FILTER_FILES, "file001"),
    reset: (ctx) => ctx.prep.input(FILTER_FILES, ""),
  },
  {
    id: "files.toggle-tree",
    group: "Files",
    title: "дерево ↔ плоский список",
    sets: ["dirty"],
    async prep(ctx) {
      await workingTree(ctx);
      if ((await ctx.attr({ sel: `button[title="Show Directories"]` }, "aria-pressed")) === "true") {
        await ctx.prep.click({ sel: `button[title="Show Flat List"]` });
      }
    },
    measure: (ctx) => ctx.measure.click({ sel: `button[title="Show Directories"]` }),
    reset: (ctx) => ctx.prep.click({ sel: `button[title="Show Flat List"]` }),
  },
  {
    id: "files.content-search",
    group: "Files",
    title: "поиск по содержимому (только IPC — в интерфейсе нет)",
    sets: ["medium", "dirty"],
    measure: (ctx) => ctx.measure.run(ctx.searchScript("revision 42"), HEAVY),
  },
  {
    id: "diff.small",
    group: "Diff",
    title: "маленький файл",
    sets: ["dirty"],
    async prep(ctx) {
      await workingTree(ctx);
      await ctx.prep.click(FILE_ROW("file0000.txt"));
    },
    measure: (ctx) => ctx.measure.click(FILE_ROW("small.txt")),
  },
  {
    id: "diff.big",
    group: "Diff",
    title: "файл на 10 000 строк",
    sets: ["dirty"],
    async prep(ctx) {
      await workingTree(ctx);
      await ctx.prep.click(FILE_ROW("small.txt"));
    },
    measure: (ctx) => ctx.measure.click(FILE_ROW("big.txt"), HEAVY),
  },
  {
    id: "diff.binary",
    group: "Diff",
    title: "бинарный файл",
    sets: ["dirty"],
    async prep(ctx) {
      await workingTree(ctx);
      await ctx.prep.click(FILE_ROW("small.txt"));
    },
    measure: (ctx) => ctx.measure.click(FILE_ROW("image.bin")),
  },
  {
    id: "diff.submodule",
    group: "Diff",
    title: "submodule",
    sets: ["submodules"],
    async prep(ctx) {
      await ctx.prep.click(COMMIT_ROW(0));
      await workingTree(ctx);
    },
    measure: (ctx) => ctx.measure.click(FILE_ROW("leaf0")),
  },
  {
    id: "changes.stage-one",
    group: "Изменения",
    title: "stage 1 файла",
    sets: ["dirty", "small"],
    async prep(ctx) {
      if (ctx.set === "small") await ctx.git.modify("src/d00/file0000.txt");
      await workingTree(ctx);
    },
    measure: (ctx) => ctx.measure.click({ ...FILE_ROW("file0000.txt", "Unstaged"), child: `.act[title="Stage"]` }),
    async reset(ctx) {
      await ctx.prep.click({ ...FILE_ROW("file0000.txt", "Staged"), child: `.act[title="Unstage"]` });
      if (ctx.set === "small") await ctx.git.restore("src/d00/file0000.txt");
    },
  },
  {
    id: "changes.unstage-one",
    group: "Изменения",
    title: "unstage 1 файла",
    sets: ["dirty"],
    async prep(ctx) {
      await workingTree(ctx);
      await ctx.prep.click({ ...FILE_ROW("file0000.txt", "Unstaged"), child: `.act[title="Stage"]` });
    },
    measure: (ctx) => ctx.measure.click({ ...FILE_ROW("file0000.txt", "Staged"), child: `.act[title="Unstage"]` }),
  },
  {
    id: "changes.stage-all",
    group: "Изменения",
    title: "stage всех (≈1 500 файлов)",
    sets: ["dirty"],
    runs: 6,
    prep: workingTree,
    measure: (ctx) => ctx.measure.click({ sel: `.pane .heading button.act`, text: "Stage all" }, HEAVY),
    reset: (ctx) => ctx.prep.click({ sel: `.pane .heading button.act`, text: "Unstage all" }, HEAVY),
  },
  {
    id: "changes.unstage-all",
    group: "Изменения",
    title: "unstage всех (≈1 500 файлов)",
    sets: ["dirty"],
    runs: 6,
    async prep(ctx) {
      await workingTree(ctx);
      await ctx.prep.click({ sel: `.pane .heading button.act`, text: "Stage all" }, HEAVY);
    },
    measure: (ctx) => ctx.measure.click({ sel: `.pane .heading button.act`, text: "Unstage all" }, HEAVY),
  },
  {
    id: "changes.commit",
    group: "Изменения",
    title: "commit (1 файл, Ctrl+Enter)",
    sets: ["small", "medium"],
    async prep(ctx) {
      await ctx.git.stage("src/d01/file0001.txt");
      await workingTree(ctx);
      await ctx.prep.input(`textarea[aria-label="Commit message"]`, `bench commit ${ctx.iteration}`);
      await ctx.prep.focus(`textarea[aria-label="Commit message"]`);
    },
    measure: (ctx) => ctx.measure.key("Enter", { modifiers: 2 }),
    reset: (ctx) => ctx.git.undoCommit("src/d01/file0001.txt"),
  },
  {
    id: "changes.stash",
    group: "Изменения",
    title: "stash (1 файл)",
    sets: ["small"],
    async prep(ctx) {
      await ctx.git.modify("src/d02/file0002.txt");
      await ctx.prep.menu("stash");
      await ctx.prep.type("bench");
    },
    measure: (ctx) => ctx.measure.key("Enter"),
    reset: (ctx) => ctx.git.stashPop("src/d02/file0002.txt"),
  },
  {
    id: "net.fetch",
    group: "Сеть (локальный remote)",
    title: "fetch",
    sets: ["network"],
    prep: (ctx) => ctx.git.remoteCommit(),
    measure: (ctx) => ctx.measure.menu("fetch", HEAVY),
  },
  {
    id: "net.pull",
    group: "Сеть (локальный remote)",
    title: "pull",
    sets: ["network"],
    prep: (ctx) => ctx.git.remoteCommit(),
    measure: (ctx) => ctx.measure.menu("pull", HEAVY),
  },
  {
    id: "net.push",
    group: "Сеть (локальный remote)",
    title: "push",
    sets: ["network"],
    prep: (ctx) => ctx.git.localCommit(),
    measure: (ctx) => ctx.measure.menu("push", HEAVY),
  },
  {
    id: "ui.context-menu",
    group: "Интерфейс",
    title: "контекстное меню коммита (до показа меню)",
    sets: ["medium", "large"],
    measure: (ctx) => ctx.measure.rightClick(COMMIT_ROW(4 + (ctx.iteration % 5)), { until: "popup_context_menu" }),
    reset: (ctx) => ctx.dismissNativeMenu(),
  },
  {
    id: "ui.preferences",
    group: "Интерфейс",
    title: "открытие Preferences",
    sets: ["medium"],
    measure: (ctx) => ctx.measure.menu("settings"),
    reset: (ctx) => ctx.prep.key("Escape"),
  },
  {
    id: "ui.theme",
    group: "Интерфейс",
    title: "смена темы",
    sets: ["medium"],
    async prep(ctx) {
      await ctx.prep.menu("settings");
      await ctx.prep.click({ sel: `button.nav-row`, text: "Theme & Colours" });
    },
    measure: (ctx) => ctx.measure.select(`.dialog select, select`, ctx.iteration % 2 === 0 ? "light" : "dark"),
    reset: (ctx) => ctx.prep.key("Escape"),
  },
  {
    id: "repo.switch",
    group: "Репозиторий",
    title: "переключение между репозиториями",
    sets: ["medium", "large", "dirty"],
    async prep(ctx) {
      await ctx.ensureOpen("small");
      await ctx.prep.click(REPO_ROW("small"), { quiet: 300 });
    },
    measure: (ctx) => ctx.measure.click(REPO_ROW(ctx.set), { quiet: 300 }),
  },
  {
    id: "repo.open-submodule",
    group: "Репозиторий",
    title: "открытие submodule",
    sets: ["submodules"],
    async prep(ctx) {
      const folded = await ctx.exists({ sel: `button.disclosure[aria-label="Show submodules"]` });
      if (folded) await ctx.prep.click({ sel: `button.disclosure[aria-label="Show submodules"]` });
    },
    measure: (ctx) => ctx.measure.click({ sel: `.row.module`, text: "libjam" }, { quiet: 300 }),
    reset: (ctx) => ctx.prep.click(REPO_ROW("submodules"), { quiet: 300 }),
  },
  {
    id: "repo.open-worktree",
    group: "Репозиторий",
    title: "переключение на worktree",
    sets: ["medium"],
    measure: (ctx) => ctx.measure.doubleClick({ sel: `[aria-label="Worktrees"] [role="option"]`, text: "medium-worktree" }, { quiet: 300 }),
    reset: (ctx) => ctx.prep.doubleClick(WORKTREE_ROW(0), { quiet: 300 }),
  },
  {
    id: "repo.close",
    group: "Репозиторий",
    title: "закрытие",
    sets: ALL,
    measure: (ctx) => ctx.measure.click({ ...REPO_ROW(ctx.set), child: `.act[title^="Close"]` }, { quiet: 300 }),
    reset: (ctx) => ctx.prep.run(ctx.dropScript(ctx.repo), { quiet: 300 }),
  },
];
