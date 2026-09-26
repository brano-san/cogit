# F-046 · Pull только fast-forward

- [ ] Кнопка Pull подтягивает изменения только перемоткой; разошедшуюся историю она отказывается сливать молча.
- [ ] В отсоединённом HEAD (и посреди rebase) Pull, Sync и `Remote ▸ Synchronise` неактивны с
      подсказкой «HEAD is not on a branch». Так же в палитре и в меню. Каретка Pull при этом
      тоже неактивна: Fetch — через `Remote ▸ Fetch` (`Ctrl+Shift+F`).
- [ ] На ветке без upstream Pull (кнопка, палитра, `Remote ▸ Pull`, строка Repositories) молча
      делает fetch всех remotes — `git fetch --all`, без ошибки и без записи Undo; Sync там
      неактивен: «The branch tracks no remote branch» (R-552).
