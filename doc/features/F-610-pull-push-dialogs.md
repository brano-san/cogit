# F-610 · Диалоги Pull и Push: теги, notes, lease, запоминаемые настройки

Большие кнопки `Pull` и `Push` тулбара (и пункты палитры `Pull…` / `Push…`) открывают диалоги на шаблоне (06 «Шаблон диалога»). Пункты `▾` **Pull with Defaults** / **Push with Defaults** (и палитра, Sync, повтор из окна ошибок, «Create Pull Request ▸ Push and Open») идут без диалога по настройкам репозитория.

## Pull

Заголовок «Pull commits from a remote repository», подзаголовок поясняет разницу с Fetch Only. **Fetch From** — список remote, под ним полный URL (`fg.muted`, переносится). Сворачиваемый **More Options** (состояние — ключ `networkDialogs.moreOpen` файла настроек):

- radio `Merge fetched remote changes` (`--ff-only` / `--no-rebase` по Preferences ▸ Pull) / `Rebase local branch onto fetched changes` (`--rebase`);
- `Update existing and fetch new tags` — `--tags --force`;
- `Fetch notes`;
- `Remember as default for repository`.

Кнопки: `Pull` (primary), `Fetch Only` (те же теги и notes, без merge/rebase), `Cancel`. Под опциями — строка `Command`: точная команда (`lib/network-dialogs.ts::pullCommandLine`).

Ветка без upstream: `Pull` только fetch (как кнопка тулбара, R-552). Конфликт merge/rebase идёт тем же путём, что у обычного Pull: потоковая команда → `record_move` → окно ошибок («ended with conflicts»).

## Push

Заголовок «Push commits to a remote repository». Списки **Remote** и **Branch** (ветки remote из tracking-refs плюс имя локальной ветки — для новой), строка `локальная → remote/ветка` целиком, с переносом. Число коммитов к отправке и раскрываемый список (первые 200, дальше «and N more»; число точное). `Set as upstream` (`--set-upstream`) включена, пока у ветки нет upstream. Теги: `No tags` / `Tags pointing to pushed commits` (`--follow-tags`) / `All tags` (`--tags`). `Push notes` и подсказка `fg.secondary`: «N notes not pushed» либо «Not compared with the remote: fetch its notes to see». `Force (with lease)`: при открытии всегда выключена, не запоминается, при включении — `Callout warning`. `Remember as default for repository` — всё, кроме Force. Кнопки `Push` (primary), `Cancel`; недопустимое имя ветки — `status` в футере.

Куда идёт «по умолчанию» (`pushTargetOf`): triangular `pushTarget`, иначе push-remote (или primary) и имя ветки upstream, если он на том же remote, иначе имя локальной ветки. Push без upstream с несколькими remote открывает этот диалог вместо Push To.

## Notes

Fetch notes кладёт чужие notes **рядом**: `git fetch +refs/notes/*:refs/notes-remote/<remote>/*`. Затем по каждому namespace:

| Состояние | Действие |
|---|---|
| локального `refs/notes/<ns>` нет | создаётся на объект remote |
| равны или remote — предок локального | ничего |
| локальный — предок remote | `git update-ref` вперёд (fast-forward, с проверкой старого значения) |
| разошлись | **не трогаются**; ответ `NotesFetch.diverged`, диалог подтверждения `Merge notes` → `git notes --ref refs/notes/<ns> merge refs/notes-remote/<remote>/<ns>`; при конфликте `git notes merge --abort`, ошибка — слова git целиком |

Push notes — отдельной командой после ветки: `git push <remote> refs/notes/*:refs/notes/*` (без force). Отказ non-fast-forward — не ошибка, а результат `PushOutcome.notesRejected` (слова git целиком); ветка уже отправлена. Диалог предлагает `Fetch and merge` → fetch notes, merge расхождений, повторная отправка notes (`push_notes`). После успешной отправки `refs/notes-remote/<remote>/*` сдвигаются на отправленное — по ним считается «N notes not pushed».

## Запоминание

Ключи локального конфига репозитория (`git config --local`): `cogit.pullMethod`, `cogit.pullTags`, `cogit.pullNotes`, `cogit.pushTags`, `cogit.pushNotes`, `cogit.pushSetUpstream` (нет ключа — «автоматически»). Force не хранится.

Логика: `lib/network-dialogs.ts` (выбор → параметры → команда, слияние defaults, проверка ветки, `pushTargetOf`), `lib/network-flow.ts` (notes: расхождение, отказ push), `stores/network-dialog.svelte.ts`. Rust: `git_engine::network_options` (`fetch_options`, `pull_options`, `push_options`, `push_notes`, `fetch_notes`, `merge_notes`, `push_preview`, `network_defaults`, `save_network_defaults`), `app_state::network_dialogs`, команды в `src-tauri/src/commands/network.rs`. Тесты: `crates/git_engine/tests/network_options.rs` (локальные bare-remote: теги, notes — fast-forward, расхождение, merge, конфликт, non-ff push, lease, set-upstream, режимы тегов, preview, defaults).
