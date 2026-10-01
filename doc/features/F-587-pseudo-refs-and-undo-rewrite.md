# F-587 · Other Refs без псевдо-refs и Undo Last Merge / Rebase / Reset

- [ ] **Other Refs.** Группа Branches ▸ Other Refs показывает настоящие нестандартные ссылки под
      `refs/`: всё, что не `heads`, `remotes`, `tags`, `stash` (`refs/notes/*`, `refs/pull/*`,
      `refs/replace/*`, …). Псевдо-refs `ORIG_HEAD`, `MERGE_HEAD`, `CHERRY_PICK_HEAD`,
      `REVERT_HEAD`, `REBASE_HEAD` (имена вне `refs/`) скрыты; если после этого группа пуста,
      её нет вовсе. `FETCH_HEAD` не показывается никогда: это список строк, не ссылка.
- [ ] **`refsShowPseudoRefs`.** `Preferences ▸ General ▸ Branches`, по умолчанию выключено:
      возвращает псевдо-refs в Other Refs. Ключ плоский camelCase, как остальные (R-631).
- [ ] **Undo Last Merge / Rebase / Reset…** (`Repository`-меню и палитра, id `undo-rewrite`).
      Вопрос в общем диалоге подтверждения: с какого коммита на какой вернётся ветка. Есть
      незакоммиченные правки отслеживаемых файлов — диалог в режиме warning и текст про них.
      Выполняется `git reset --keep ORIG_HEAD` через `git_engine`, в очереди операций репозитория,
      пишется в журнал безопасности (его Undo возвращает обратно). Нет `ORIG_HEAD` или он равен
      HEAD — отказ с объяснением, git не запускается. Кнопка `Undo` в тулбаре — прежнее Undo
      журнала, не это действие (R-631).
- [ ] **Compare Before and After Rewrite** читает `ORIG_HEAD` напрямую (`git range-diff
      ORIG_HEAD...HEAD`), поэтому не зависит от видимости в списке (тест
      `compare_before_and_after_rewrite_reads_orig_head_without_the_list`).
- [ ] Команды IPC: `undo_rewrite_info`, `undo_rewrite` (04-ipc-contract).
