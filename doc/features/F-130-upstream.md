# F-130 · Управление upstream

- [ ] У локальной ветки в меню Branches (#33) и в меню её метки в графе (#39) — `Set Upstream…`
  и `Stop Tracking`, сразу под `Push` / `Push To…` (R-505). `Set Upstream…` открывает диалог на
  общем `Dialog`: выбор remote-ветки (сначала текущий upstream, затем ветка того же имени на
  основном remote), кнопка неактивна с причиной, если выбран текущий upstream или remote-веток
  нет. Запись — `git branch --set-upstream-to <remote>/<ветка> <ветка>`; `Stop Tracking` —
  `git branch --unset-upstream <ветка>`, обе через очередь записей (`set_upstream`).
  Неактивно с причиной: у remote-ветки и тега — оба пункта, без remote — `Set Upstream…`, без
  upstream — `Stop Tracking`. Тесты — `ref-menus.test.ts` «upstream rows (F-130)»,
  `upstream.test.ts`, `crates/git_engine/tests/branch_ops.rs`
  `setting_an_upstream_makes_the_branch_track_it`, `crates/app_state/tests/handles.rs`
  `an_upstream_set_or_stopped_here_is_read_back`.
