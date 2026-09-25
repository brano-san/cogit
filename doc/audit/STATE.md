# Аудит 25.09.2026 — состояние работы

Источник истины для задачи «полный аудит, проверка функционала и исправления». После паузы
или сжатия контекста — сначала этот файл, затем `findings.md`.

## Режим

- Ветка `audit/2026-09-25` от `master` (a779a38), без push.
- Тяжёлые команды — `cmd //c 'D:\cogit-work\heavy.cmd' <команда>` (ядра 16–31; worktree
  `D:\cogit-work\wN` собирает в свой `D:\cogit-work\build-wN`). Сторож памяти —
  `D:\cogit-work\memwatch.ps1` (лог `memwatch.log`); тесты на 50k-фикстурах — ≤ 4 потоков.
- Бенчмарк — только на простаивающей машине. База «до» — `target/bench/exes/tasks-final3.exe`
  (a6b23f1; код совпадает с a779a38).

## Области аудита (фаза 1)

| Ключ | Префикс | Область | Файлы (владение для аудита) |
|---|---|---|---|
| git-engine | GE | чтение gix / запись git, ошибки, пограничные репозитории | `crates/git_engine/src/**` кроме `graph_walk.rs`, `topo.rs` |
| graph | GR | инварианты полос, ленивое чтение, обрубки, режимы | `crates/graph_engine/**`, `git_engine/src/{graph_walk,topo}.rs`, `app_state/src/graph_*.rs`, `GraphCanvas.svelte`, `FoldToggle.svelte`, `lib/graph-{geometry,links,modes,mode-conflicts,style,wire,anchor}.ts`, `stores/graph*.svelte.ts` |
| diff | DF | окончания строк, переименования, submodules, большие и бинарные | `crates/diff_engine/**`, `app_state/src/diffing.rs`, `components/diff/**`, `panels/DiffPanel.svelte`, `lib/{diff-*,highlight,merge-*,compare-params}.ts`, `stores/{diff,compare-view,conflicts}.svelte.ts`, `MergeWindow`, `CompareWindow` |
| backend | BE | команды, события, DTO против `doc/04`, bindings | `crates/app_state/src/**` (кроме graph_*, diffing), `src-tauri/**`, `crates/{fs_watcher,avatars,build_info}`, `frontend/src/lib/ipc/**` |
| concurrency | CC | await под блокировкой, порядок блокировок, устаревшие ответы, отмена, очередь, watcher, `unsafe` | сквозная, только чтение, файлов не владеет |
| fe-repos | FR | деревья, чекбоксы, сортировка, индикаторы, submodules | `components/{repo-tree,branch-tree,remote}/**`, `panels/{Repositories,References,Worktrees}Panel.svelte`, `stores/{repo-list,repo-groups,repo-pulse,refs,worktrees,submodules,module-forest,module-memory,scan,flow,stashes}`, `lib/{repo-*,ref-*,module-*,pulse-queue,worktree-list,drop-open,tree,tri-state-box,remote-*,push-to,pull-request}.ts` |
| fe-graph-files | FG | выбор, фокус, усечение, фильтры, поиск | `components/graph/**` кроме GraphCanvas/FoldToggle, `components/file-list/**`, `panels/{Graph,Files,Commit}Panel.svelte`, `CommitDetailsPane.svelte`, `stores/{files-view,commit-tree,commit,worktree,finder,stash-view,stash-dialog,overlap}`, `lib/{graph-row,graph-columns,graph-panel,file-*,files,commit-*,selection,multi-select,stage-all,content-search,query,skipped,split-off,rebase-plan,rewrite-plans,overlap,truncate,staleness,disk-change,mutation}.ts` |
| fe-shell | FS | тулбар, меню, диалоги, Preferences, уведомления, окна | `App.svelte`, `main.ts`, окна Blame/Investigate, `components/{layout,common,menus,investigate}/**`, остальные `stores/*` и `lib/*` фронтенда |
| docs | DC | документы против кода | `doc/01`, `03`, `04`, `07`, `11`, `doc/features/**` |
| tests | TS | покрытие критичных путей, флапающие тесты, паритет с git | `crates/*/tests/**`, `crates/test_fixtures`, `frontend/**/*.test.ts`, `scripts/bench/**` |

## Фазы

- [x] Фаза 1 — аудит (workflow 1): `doc/audit/<ключ>.md`, два прохода на область (второй проход
  записан в файл только у git-engine; у остальных — в `findings.md` через синтез).
- [x] Фаза 2 — проверка находок другим агентом (critical/high — двумя, спор — третьим), слияние
  дублей, владельцы, `findings.md`: 388 заявлено → 299 подтверждённых после слияния 70 дублей,
  51 ux — в предложения, 4 спорных, 13 снятых, 28 групп исправления.
- [~] Фаза 3 — статусы 310 фич готовы (`features.md`: тест 247, автоматизация 2, только по коду 12,
  сломано 49); база тестов снята (см. журнал); тесты для 21 критичного пути — в фазе 4 владельцами
  групп; бенчмарк «до» — `tasks-final3.exe`, мерится в той же сессии A/B, что и «после».
- [ ] Фаза 4 — исправления по порядку задачи; после — полный прогон и бенчмарк.
- [ ] Итог в конце `findings.md`.

## Журнал

- 25.09 — ветка, окружение, базовый полный прогон тестов запущен (`D:\cogit-work\base-tests.log`).
- 25.09 — база тестов (a779a38): nextest 1934/1934 (5 skipped), cogit lib 108, vitest 1932 в 154
  файлах, svelte-check 0/0, clippy чисто.
- 25.09 — workflow 1 (фазы 1–3, только чтение): run `wf_f2e206a0-103`, скрипт
  `…/workflows/scripts/cogit-audit-phase-1-3-wf_f2e206a0-103.js`; возобновление —
  `Workflow({scriptPath, resumeFromRunId: "wf_f2e206a0-103", args: {features: <D:\cogit-work\features.json>}})`.
- 25.09 01:20 — лимит сессии: готовы 19 агентов из 92 (первый проход всех 10 областей — 256
  находок в `doc/audit/<ключ>.md`, пачки фич 1–9); второй проход, проверка, пачка 10 и синтез
  упали. 01:51 — возобновлено тем же run id.
- 25.09 02:05 — снимок перед лимитом. Готово: проход 1 всех 10 областей (256 находок: git-engine
  27, graph 14, diff 35, backend 25, concurrency 15, fe-repos 30, fe-graph-files 45, fe-shell 24,
  docs 20, tests 21 — в `doc/audit/<ключ>.md`); статусы 279 из 310 фич (тест 218, автоматизация 1,
  только по коду 14, сломано 46; критичные пути без теста — F-035, F-040, F-042, F-045, F-047,
  F-056, F-071, F-091, F-132, F-135, F-140, F-147, F-212, F-229, F-242, F-257, F-311, F-320,
  F-333) — пока только в журнале workflow (`…/subagents/workflows/wf_f2e206a0-103/journal.jsonl`).
  Идёт: проход 2, проверка, пачка фич 10, синтез. Дальше — фаза 3 (тесты критичных путей),
  фаза 4.
- 25.09 ~04:00 — второй лимит: готово 63 агента (оба прохода всех 10 областей — 388 находок,
  статусы всех 310 фич: тест 247, автоматизация 2, только по коду 12, сломано 49), часть
  проверок; упали остальные проверки и синтез. 16:22 — возобновлено тем же run id, сторож
  памяти перезапущен.
- 25.09 17:21 — по журналу: 388 находок, подтверждено 363 (high 17, medium 124, low 222;
  critical нет), снято 10, спорных 5, 10 ещё проверяются. Возобновлено для последних 12 агентов.
- 25.09 — **решение пользователя об объёме фазы 4: high + medium (~141)**; low — в «отложено»,
  кроме тривиальных в тех же файлах. После аудита — без ultracode (экономия токенов): правки
  делаю сам, отдельные агенты — точечно.
- 25.09 17:50 — фаза 4, волна 1: 4 агента в `D:\cogit-work\w1…w4` (ветки `audit-fix/wN`),
  бриф `D:\cogit-work\BRIEF4.md`, раскладка и следующие волны — `D:\cogit-work\agents.md` и
  `plan4.json`. В объёме high+medium после слияния дублей — 124 находки (19 high, 105 medium):
  w1 18, w2 20, w3 12, w4 21; волна 2 — s1 14, s2 10; волна 3 — s3 29 (App.svelte).
- 25.09 — w3 закончил (12/12 + 3 low + 4 файла фич) и слит: 02c06fc; отчёт —
  `D:\cogit-work\reports\w3.md`.
- 25.09 — **ПАУЗА по просьбе пользователя.** Агенты w1, w2, w4 остановлены посреди работы; их
  коммиты целы на ветках: w1 — 22 коммита (+ в индексе незакоммиченные `git_engine/src/health.rs`
  и `tests/health.rs`), w2 — 25, w4 — 27. Cron `ac7bd580` и сторож памяти сняты.
  **Как продолжить:** запустить сторож памяти; каждому из w1, w2, w4 — SendMessage по id из
  `D:\cogit-work\agents.md`: «Работа прерывалась: проверь git status/log своего worktree, доведи
  или откати незакоммиченное и продолжи по брифу с первой несделанной находки»; по отчётам — слить
  ветки, затем волны 2 и 3 по `agents.md`.
- 25.09 — автопродолжение фазы 4 (снято): cron `ac7bd580` (:07, :37) — возобновляет оборванных агентов,
  сливает готовые ветки, запускает следующую волну. Старый cron `cd4b5551` снят.
- 25.09 — автопродолжение (снято): cron `cd4b5551` (:07 и :37 каждого часа) проверяет workflow и
  возобновляет его после сброса лимита; снять — CronDelete, когда аудит закончен.
