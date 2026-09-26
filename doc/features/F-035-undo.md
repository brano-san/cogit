# F-035 · Undo деструктивных операций

- [ ] Кнопка Undo в тулбаре возвращает отброшенные изменения и удалённую ветку ровно в то состояние, что было до операции.
  Копии для Undo — Discard, Reset Hard, Roll Back, Remove Worktree с `--force` — лежат в
  `refs/cogit/backup/<oid>`, а не в списке stash: список пользователя, его номера и `git stash pop`
  их не видят; Undo применяет копию и снимает её ссылку, Undo отката новой записи в stash не
  добавляет (R-514). Тесты — `crates/app_state/tests/undo.rs`
  `the_copies_undo_keeps_stay_out_of_the_stash_list`, `a_hard_reset_keeps_its_copy_out_of_the_stash_list`,
  `undoing_a_rollback_adds_nothing_to_the_stash_list`.
