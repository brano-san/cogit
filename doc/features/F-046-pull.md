# F-046 · Pull только fast-forward

- [ ] Кнопка Pull подтягивает изменения только перемоткой; разошедшуюся историю она отказывается сливать молча.
- [ ] В отсоединённом HEAD (и посреди rebase) Pull, Sync и `Remote ▸ Synchronise` неактивны с
      подсказкой «HEAD is not on a branch»; на ветке без upstream — «The branch tracks no remote
      branch». Так же в палитре и в меню. Каретка Pull при этом тоже неактивна: Fetch — через
      `Remote ▸ Fetch` (`Ctrl+Shift+F`).
