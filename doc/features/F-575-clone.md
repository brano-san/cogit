# F-575 · Clone: мастер из трёх страниц

Сверено с документацией SmartGit: [Cloning Repositories](https://docs.syntevo.com/SmartGit/Latest/Manual/GUI/Repository/Clone)
и [Partial Clones (23.1)](https://docs.syntevo.com/SmartGit/23.1/Manual/Clone) — те же страницы
Repository, Selection, Directory и те же флажки; мелкого клона (`Fetch only the latest N commits`)
и хостинг-провайдеров у Cogit нет (R-600).

- [ ] `Repository ▸ Clone…` и палитра («Clone Repository…») открывают мастер `Clone` на общем
      `Dialog`: сверху шаги `1. Repository · 2. Selection · 3. Directory`, внизу `Cancel`, `Back`
      (на первой странице неактивна), `Next` / `Finish`. `Enter` — Next или Finish, `Esc` и ✕ —
      закрыть; если введён адрес или пройдена первая страница — сначала вопрос «Discard Changes».
      Сочетания клавиш нет.
- [ ] **Repository.** Поле «Repository URL or local folder» и `Browse…` (выбор папки). В буфере
      обмена ссылка на репозиторий (`https://…/x.git`, `https://github.com/owner/name`,
      `git@host:owner/name.git`, `ssh://`, `git://`, `file://`) — она уже в поле при открытии;
      любой другой текст буфера на страницу не попадает (`clipboard_repository_url`, R-600).
      Пусто, относительный путь, адрес с `-` в начале — `Next` неактивна, причина слева внизу.
- [ ] `Next` проверяет доступ: `git ls-remote --symref -- <url> HEAD refs/heads/*` без записи и без
      запросов — как фоновая проверка (R-353, R-354): ни терминала, ни askpass, ни окна Credential
      Manager, ssh в `BatchMode`; сохранённые учётные данные и токен Cogit для хоста работают. Пока
      идёт проверка — «Checking access to the repository…». Отказ — сырой вывод git на странице и
      запись в Output; кнопка `Continue Without Check` ведёт дальше без списка веток (войти можно
      будет в окне Credential Manager во время самого клона). Правка адреса снимает отказ.
- [ ] **Selection.** `Include submodules` и `Fetch all heads and tags` включены,
      `Skip large files (partial clone)` выключен, рядом «Omit files larger than [1] MB» (активно
      только с флажком). «Check out branch» — ветки сервера из ответа проверки, ветка его HEAD первой
      с пометкой `(default)`; без проверки или у пустого репозитория список неактивен с причиной.
- [ ] **Directory.** «Parent folder» (по умолчанию — папка, где лежит открытый репозиторий, иначе
      последний из недавних) с `Browse…`, «Folder name» (по умолчанию — из адреса, как у
      `git clone`; своё имя адресом больше не меняется), строка «Clone into <путь>». Папка
      непустая, это файл или не читается — `Finish` неактивна с причиной «<путь> is not empty».
- [ ] `Finish` закрывает мастер; клон — операция очереди `clone` в своей полосе (по папке
      назначения), в футере «Cloning…» и последняя строка прогресса git, `Cancel` в футере его
      останавливает (`cancel_network`); отменённый клон убирает созданную им папку, пустую папку
      оставляет пустой (R-602). Удачный — репозиторий открыт и в списке Repositories, как после
      Open. Ошибка — обычное окно ошибки с выводом git.
- [ ] Команда: `git clone --progress [--recurse-submodules] [--single-branch] [--branch <b>]
      [--filter=blob:limit=<N>m [--also-filter-submodules]] -- <url> <папка>`; `--branch` — только
      для ветки, отличной от HEAD сервера. Partial clone — `blob:limit`, не `blob:none` (R-601).

Тесты: `crates/git_engine/tests/cloning.rs` (проверка и паритет с `git ls-remote --heads`, пустой
репозиторий, отказ с выводом git в журнале, клон всех веток, одной ветки, без submodules, partial
clone, непустая папка, отмена — папка убрана / пустая осталась пустой, распознавание ссылок в
буфере), `cloning_submodules.rs` (с submodules), `crates/app_state/tests/cloning.rs` (своя полоса без
репозитория, журнал, отмена из футера), `src-tauri/src/commands/tests.rs`
`every_command_that_talks_to_a_remote_can_be_cancelled`, `menu/mod.rs`
`clone_follows_open_in_the_repository_menu`; `frontend/src/lib/clone.test.ts`,
`stores/clone.test.ts`, `stores/network.test.ts` «keeps a clone's line», `lib/exit.test.ts`,
`lib/operations.test.ts`.
