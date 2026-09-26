# F-556 · Диалог Apply Stash

- [ ] Двойной клик по stash-у в Branches, по его метке в графе и `Apply Stash` в меню stash-а
      открывают диалог `Apply Stash`: `Cancel`, `Apply`, `Apply & Drop` (по умолчанию) и
      чекбокс `Restore Index` — staged-правки возвращаются staged (`stash apply --index`).
      `Apply & Drop` — `stash pop`: stash удаляется, только если применился без конфликтов
      (R-562). `Pop Stash` меню применяет и удаляет сразу, без диалога.
      Тесты — `crates/git_engine/tests/stashes.rs` `restore_index_brings_staged_changes_back_staged`,
      `apply_and_drop_restoring_the_index_removes_the_stash`; `stashes.test.ts`.
