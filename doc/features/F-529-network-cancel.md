# F-529 · Отмена сетевой операции

- [ ] Идущий fetch, pull или push (и Push To, Push Up To) отменяется по `id` своей операции
  очереди: `cancel_network(operation)` останавливает дерево процессов git, вызов отклоняется
  с `GitError::Cancelled`, очередь репозитория идёт дальше, в Output — предупреждение
  «Canceled by the user»; окно уведомлений отмену не показывает (R-506). `false` — отменять
  нечего.
- [ ] Пока идёт fetch, pull или push, в футере рядом с индикатором — кнопка `Cancel`: она
  останавливает последнюю начатую из них — ту, что называет индикатор; ждущая в очереди
  отмены не получает, git у неё ещё не запущен. Какие идут — по `operation-changed`
  (`kind`, `phase`), `lib/operations.ts` `trackCancellable`, `cancellable`.
  Тесты — `crates/git_engine/tests/network.rs` `a_fetch_asked_to_stop_ends_as_cancelled`,
  `a_fetch_stopped_before_it_starts_never_runs_git`; `crates/app_state/tests/cancel_network.rs`;
  `src-tauri/src/commands/tests.rs` `every_command_that_talks_to_a_remote_can_be_cancelled`;
  `notices.test.ts` «says nothing about a network command the user cancelled»;
  `operations.test.ts` «trackCancellable».
