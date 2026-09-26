# F-039 · Предупреждение о блокировке индекса

- [ ] Оставшийся `index.lock` показывается красной плашкой с полным путём поверх всех остальных состояний.
  Наблюдатель слышит, как lock появляется и исчезает (`ChangeKind::Index`), а `working_state`
  несёт `indexLock`: плашка приходит и уходит без `F5` и без движения ссылок (R-507). Тесты —
  `crates/fs_watcher/tests/watching.rs` `an_index_lock_coming_and_going_is_an_index_change`,
  `crates/git_engine/tests/repo_state.rs` `the_working_state_carries_the_index_lock`,
  `repository.test.ts` «shows an index.lock that appeared and drops one that went».
