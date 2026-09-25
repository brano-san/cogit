# F-071 · Удаление с диска

- [ ] Кнопка Delete над Unstaged и пункт `Delete…` после подтверждения переносят файлы в Корзину
  (R-266). Перед этим каждый файл — отслеживаемый с правками или неотслеживаемый — копируется в
  хранилище объектов, и в журнале появляется запись с Undo: он пишет файлы обратно, но не
  поверх появившегося снова. Папка остаётся на Корзину: запись есть, Undo нет (R-448).
  Тесты — `crates/app_state/tests/undo.rs`: `deleting_a_changed_file_can_be_undone`,
  `undoing_a_delete_never_writes_over_a_file_made_since`, `a_deleted_folder_is_left_to_the_bin`.
