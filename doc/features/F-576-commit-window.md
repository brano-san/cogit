# F-576 · Окно Commit (Ctrl+K)

- [ ] `Ctrl+K` открывает отдельное окно Commit по образцу SmartGit; `Ctrl+5` по-прежнему показывает панель Commit Message.

Окно — дочернее (`commit.html`, как Blame и Investigate): без меню, тема и фон как у главного, закрывается кнопкой Cancel, `Esc` и `Ctrl+W`, ничего не меняет в главном окне.

- Радио Staged Changes / Local Changes; справа счётчик `1 file (~1)` (`+` новые, `~` изменённые, `-` удалённые, `>` переименованные) и значок с подсказкой.
- Таблица с галкой у каждого файла, колонки Name (значок состояния) и Directory, сортировка по заголовку; галка в заголовке — все.
- Commit Message с меню Select: «Select from Log…» (выбор из последних 100 коммитов), «Pending Message» (сохранённый черновик), затем последние сообщения.
- Amend last commit (подставляет сообщение последнего коммита, при снятии возвращает набранное), More Options → `Signed-off-by` (`--signoff`) и `--no-verify`. Подпункты скрыты, пока галка не стоит.
- Commit, Commit & Push, Cancel. Commit выключен без сообщения или без отмеченных файлов; `Ctrl+Enter` = Commit. Amend опубликованного коммита спрашивает, как и панель.
- Черновик сообщения — тот же `cogit:draft:<root>`, что у панели: оба окна слушают `storage`.
- Local Changes: неиндексированные отмеченные файлы сначала `git add`, затем коммит `--only`-путями (scratch-индекс, `commit_write.rs`): файл, проиндексированный частично, уходит целиком отмеченным.
- Логика коммита общая: `worktree.commit`, `CommitRequest`, `canCommit` (`commit-draft.ts`); новый код — `lib/commit-window.ts`.
