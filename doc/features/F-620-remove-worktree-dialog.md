# F-620 · Диалог Remove Worktree: этапы проверки, потери данных

Пересобран на шаблоне диалога (06 «Шаблон диалога»): карточка (Worktree, Branch, Path) → этапы проверки и список → callout-блоки → опция → футер. Кнопка удаления деструктивная (`status.danger`), фокус по умолчанию на `Cancel`.

- **Размер сразу**: диалог открывается готовой формы. Пока идёт проверка, вместо списка — скелетон-строки и «Checking uncommitted changes…»; кэшированные `dirty`/`hasSubmodules` из списка Worktrees уже рисуют warning- и danger-callout и галочку `--force` (неактивную), так что размер почти не меняется, когда придёт ответ.
- **Три этапа** (`lib/worktree-scan.ts`, `stageRows`): `Changes`, `Submodules`, `Unpushed commits` — у каждого индикатор (идёт / готов / ошибка) и итог («4 files», «2 checked out», «6 in 2 submodules», «none»). Идут параллельно, отвечают в любом порядке; ошибка этапа показывается в его строке, `Remove` остаётся неактивной («The check failed»).
- **Список «Uncommitted changes (N)»**: правки и изменённые submodules вместе по пути, иконки и текст состояния — `fileState` (F-600). Показано 40, остаток — «and N more».
- **Callout `status.warning`** — незакоммиченные изменения будут потеряны; **`status.danger`** — submodules выписаны (нужен `--force`, их репозитории удаляются) и отдельный danger со списком **неотправленных коммитов** по submodules (число, 3 последних «хэш тема»): stash их не сохраняет.
- **Опция** `Remove anyway (--force)` с подсказкой «Changes are stashed first; Undo brings them back» (у чистого worktree с submodules — «Git refuses to remove a worktree with submodules without it»).
- **Кнопка** `Remove` / `Remove with --force`; неактивна с tooltip «Waiting for the check» до конца проверки, «Tick Remove anyway (--force) first» — пока не отмечена галочка.
- **Закрытие** отменяет чтение (`cancel_operation`), ответы закрытого диалога отбрасываются.

Rust: `git_engine::{WorktreeScan (changes / submodules / unpushed), WorktreeSubmodules, UnpushedInSubmodule}`, `RepoHandle::worktree_scan`, тесты `crates/git_engine/tests/worktree_scan.rs`; IPC `scan_worktree_removal` (04), оркестрация и логирование этапов в `src-tauri/src/commands/worktrees.rs`. Фронт: `lib/worktree-scan.ts` (машина состояний этапов и кнопки, тест рядом), `stores/worktrees.svelte.ts` (`scanRemoval`). Замеры и отвергнутые варианты — R-675; стенд — `crates/git_engine/tests/scan_bench.rs`.
