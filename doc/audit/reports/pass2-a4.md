# Проход 2 · a4 — G-06, G-08, G-09 (10 коммитов, последний 9628530)

Medium: FR-010 6a7e279 (группы с совпадениями раскрыты при фильтре, свёртка при фильтре — отдельно; R-485, F-144)
Low: GE-032 b90a0a9 (stash выделения рядом со staged-удалением; R-486) · TS-016 6b798a8 · GE-013 dcf5595 (worktree_heads() без статуса; R-487; без падающего теста — это скорость) · FX-007 уже 82a5a7b · FX-009 6ffe778 (Update в меню строки submodule; R-488, F-521) · FR-012 90c5c3e (perf: tickStates за один проход; R-489) · FR-021 a342f90 · FR-030 1d5c418
Лёгкая галочка: 5268fdd + 9628530 (perf; класс tick-box, appearance:none, ::before с clip-path; R-455, doc/06) — A/B и откат вместе. Проверить в сборке: галочки Branches в 4 темах, mixed, выключенный тег, фокус.
A/B нужен: 90c5c3e (FR-012), dcf5595 (GE-013), пара 5268fdd+9628530.
Провод для b3 (App.svelte), FX-009: (1) moduleContext передаёт `module: row.module` в repoMenu; (2) в runRepoCommand `case "repo-update"` для target.kind === "submodule": `void updateModule(target.row.module, { ask: (request) => confirmation.ask(request), update: () => refreshSubmodule(target.row) })`; (3) чужая строка (target.top): openRepository(top) → repoList.opened → submodules.own(owner.repo, owner.root), затем refreshSubmodule по строке submodules.rows с тем же key.
UX-предложения: виртуализация дерева Branches (R-489) — если A/B покажет нужду; Update у строки репозитория как `submodule update --init --recursive`.
Тесты: vitest 2195/2195; git_engine+fs_watcher 139/139; app_state 90/90; хук.
Конфликты: 12-risks (R-485…489, R-455), features/README (F-521), 06, app.css, RefTree, RepositoryList, ref-nodes, tree.ts, repo-menu, module-tree, app_state lib.rs (discard_paths), worktrees.rs, git_engine stash/worktrees/ancestry.
