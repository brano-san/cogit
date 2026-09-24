# Ночной рефакторинг, 2026-09-24

Источник истины для автономной работы. После сжатия контекста или авто-продолжения —
сначала перечитать этот файл, потом продолжать со следующего неотмеченного шага.

## Задание (сжато)

Автономно, без вопросов; спорные решения — самому, одной строкой здесь.

- Без push, force и переписывания истории.
- Шаг: правка → затронутые тесты → clippy → коммит (этот файл — в том же коммите) → отметка
  `[x]` и строка `> Итог:`. Три неудачные попытки → `не сделано: причина`, дальше.
- Поведение не меняется. Найденный баг — отдельный коммит `fix(...)` с тестом, который его
  воспроизводит. Тесты не удалять и не ослаблять; флап тестов с замером времени — не
  регрессия, отметить здесь.
- Фаза 0 — полный прогон и бенчмарк (`doc/15-benchmark.md`). Фаза 1 — аудит (≤ ~15 %
  работы), категории: 1 гонки, 2 баги/несостыковки, 3 неиспользуемое, 4 дубли,
  5 упрощение, 6 модули. Фаза 2 — исправление в порядке 1 → 6; перенос кода — отдельным
  коммитом без логики; у правок 3–6 — польза одной строкой, без пользы не делать, без
  переименований по вкусу, без новых зависимостей. Фаза 3 — прогон и бенчмарк, сравнение,
  регрессия скорости → найти коммит и откатить; итоговые таблицы (ниже).

## Режим

- Ветка `refactor/night` от `master` 2e4ef36 (задание говорило «ветка уже отдельная», но
  текущей была `master` — отделил, чтобы не коммитить в основную).
- Тяжёлое — на ядрах 16–31: `cmd //c <scratchpad>\aff.cmd <команда>` (внутри
  `start /affinity FFFF0000 /b /wait cmd /c`); бенчмарк — `--hidden --cores 16-31`.
- Коммиты — одна строка без трейлеров (правило проекта и пользователя важнее атрибуции).
- В сессии стоит cron (:17 и :47) с подсказкой «перечитай план и продолжай» — на случай
  лимита. Когда фаза 3 отмечена — удалить.
- Не запускать LTO-сборку бенчмарка параллельно с агентами: первая попытка упала с
  `Allocation failed` в rustc (`codegen-units=1`, fat LTO).
- **Сбой с чужим stash (на решение пользователя).** Проверяя «тест падает без правки», я
  сделал `git stash push -- <файл>` по файлу без изменений (stash не создался) и `git stash
  pop` — снялся старый stash пользователя «botched bulk comment strip» с 39 конфликтами.
  `reset --hard`/`checkout HEAD --` классификатор запретил; вместо этого `git add -A` +
  `git stash push`: сейчас `stash@{0}` — это неудачное наложение, `stash@{1}` — исходный
  stash, нетронутый. Резервная копия 39 файлов — `scratchpad/stash-pop-backup/`. Можно
  удалить `stash@{0}` (`git stash drop stash@{0}`), исходный останется. Больше stash не
  использую: падение теста показываю порядком TDD.
- Первая сессия оборвалась на аудите (процесс закрылся, агенты и бенчмарк остановлены);
  продолжено в той же ветке.

## Фаза 0 — базовая точка

- [x] Полный прогон тестов до правок
  > Итог: nextest 1645 ✓ / 1 skipped (74 с); `cargo test -p cogit --lib` 100 ✓; vitest 1653 ✓
  > (121 файл); svelte-check 0 ошибок / 0 предупреждений; clippy `-D warnings` чисто.
- [x] Бенчмарк до правок; exe сохранён как `target/bench/exes/night-base.exe` для A/B в фазе 3
  > Итог: полный прогон `--hidden --cores 16-31` на 2e4ef36,
  > `doc/benchmarks/2026-09-24-night-base.json`. Первая попытка сборки упала (`Allocation
  > failed` в rustc при шести агентах), вторая — первый запуск свежего exe не дождался
  > антивируса (у сценария запуска нет повтора, как у `session`), третья прошла. Шёл
  > параллельно с моими тестами на тех же ядрах: 34 сценария `⚠ noisy`, 6 холодных запусков
  > не дождались страницы — поэтому сравнение в фазе 3 — A/B `night-base.exe` против
  > итоговой сборки в одной сессии, а не разность прогонов. Для ориентира (медиана, мс):
  > app.ready medium cold 384; repo.open large warm 279; graph.full-layout large warm 274;
  > repo.switch large warm 266; changes.stage-all dirty warm 321; changes.commit medium warm
  > 143; files.status medium warm 12,6.

## Фаза 1 — аудит

Формат: `id · категория · файл:строка · что не так · как исправить · риск`.

- [x] 1. Гонки и конкурентность
- [x] 2. Баги и несостыковки
- [x] 3. Неиспользуемый код
- [x] 4. Дубли
- [x] 5. Упрощение
- [x] 6. Разнесение по модулям
  > Итог: шесть параллельных агентов только на чтение, находки проверены выборочно по коду
  > (C1F-01…04, 08 — вручную); всего ~150 пунктов, ниже.

Miri: в `graph_engine` и `diff_engine` нет ни одного `unsafe`, nightly не установлен, на C
мало места — не запускал; весь `unsafe` в `src-tauri` (COM, subclass окна), там Miri
неприменим — инварианты проверены чтением (ниже).

### Находки

#### 1. Гонки — фронтенд

Корень C1F-01…08 один: `id` текущего репозитория захвачен до `await`, после — запись без
проверки, что репозиторий всё ещё тот же.

- C1F-01 · гонки · stores/repository.svelte.ts:181 · `refresh()` во время `opening` B берёт
  `current` = прежний A, выдаёт новый ticket и отменяет открытие B; клик по B теряется
  (триггер: `git commit` в терминале → watcher → `applyDiskChanges`) · в `refresh` ничего
  не делать, пока идёт `opening` · низкий · проверено
- C1F-02 · гонки · App.svelte:972 `applyDiskChanges` · после await зовёт `worktree.load(id)`
  / `graph.load(id)` для прежнего репозитория — у сторов побеждает последний запуск ·
  проверка «репозиторий всё ещё текущий» после каждого await · низкий · проверено
- C1F-03 · гонки · App.svelte:421 `mutate`, :1065 `commitStaged`, :1094 `afterRefChange` ·
  то же после медленного шага (pre-commit хук) · тот же guard · низкий · проверено
- C1F-04 · гонки · App.svelte:1427 `runNetwork` · fetch в A, переход в B → `afterRefChange`
  чистит выделение и дифф B · guard по id · низкий · проверено
- C1F-05 · гонки · stores/stashes, recovery, network, flow, conflicts; App `refreshProgress`,
  `loadTemplate` · результат IPC пишется без поколения; stash’и/прогресс rebase/шаблон A
  в панелях B · поколение, которое `clear()` увеличивает · средний
- C1F-06 · гонки · stores/worktrees.svelte.ts:30 · `entries` после await без guard ·
  поколение · низкий
- C1F-07 · гонки · stores/submodules.svelte.ts:53 · `own()` пишет `children` после await,
  цикл `#load` читает уже новый `#repo` · проверка root после каждого await · средний
- C1F-08 · гонки · App.svelte:1650 `activate` · после перебитого `open(A)` продолжает с
  `current` = прежний/следующий репозиторий · `open` сообщает, выиграл ли его ticket;
  проигравший `activate` выходит · низкий · проверено
- C1F-09 · гонки · stores/diff.svelte.ts:89 · `images` после второго await без поколения ·
  проверка поколения · низкий
- C1F-10 · гонки · lib/investigate/session.svelte.ts:215 · пропуск загрузки blame не
  увеличивает поколение → Back во время загрузки оставляет blame другой строки · `++gen`
  при пропуске · низкий
- C1F-11 · гонки · lib/investigate/session.svelte.ts:97 · `navigate` без токена · токен ·
  низкий · вероятно
- C1F-12 · гонки · stores/hooks.svelte.ts:50 · `edit()`: тело хука A в редакторе B, Save
  пишет его в B · писать `body` только при `editing === name` · низкий
- C1F-13 · гонки · stores/repository.svelte.ts:186 · `refreshList` без ticket, пишет в
  сессию · ticket · низкий · вероятно
- C1F-14 · гонки · stores/refs.svelte.ts:112 · `loadDates`/`loadUrls` без поколения ·
  поколение в `clear()` · низкий
- C1F-15 · гонки · components/graph/CommitList.svelte:176 · `graph.entry(t).then(pick)` без
  токена — выделение прыгает после ↑ · счётчик · низкий
- C1F-16 · гонки · stores/avatars.svelte.ts:17 · `apply(false)` не гасит таймер и запрос ·
  низкий · вероятно
- C1F-17 · гонки · App.svelte:1136 · клик по коммиту не сбрасывает `stashView`, Files
  показывает stash · `stashView.clear()` при выборе коммита · низкий
- C1F-18 · гонки · components/layout/CommandOutput.svelte:93 · drag снимается только на
  `pointerup` · и на `pointercancel`/`lostpointercapture` · низкий · вероятно

#### 1. Гонки — бэкенд

- C1-01 · гонки · app_state/src/queue.rs:153 · место в очереди берётся при первом poll, а не
  при invoke; два быстрых invoke могут выполниться в обратном порядке · `seq` с фронта или
  синхронный билет · высокий · по конструкции
- C1-02 · гонки · src-tauri/src/lib.rs:461 · `while let Ok(..) = recv()` — `Lagged` навсегда
  глушит форвардер событий (RepoChanged, Operation…) · `Lagged` → warn и дальше · низкий ·
  проверено
- C1-03 · гонки · app_state/src/lib.rs:311 · `disable_avatars` дропает очередь (join воркеров
  до ~10 с) под write-локом и на воркере tokio · `take()` под локом, drop вне его, в
  blocking · низкий · проверено
- C1-04 · гонки · avatars/src/cache.rs:200 · `save()` пишет индекс без лока и не атомарно
  из 4 воркеров · мьютекс записи + tmp/rename · низкий
- C1-05 · гонки · avatars/src/queue.rs:83 · файловый IO под `state`-локом · вынести IO за
  лок · низкий
- C1-06 · гонки · app_state/src/lib.rs:1278 `row_for` · промах → медленное чтение → insert
  поверх инвалидации · поколение на репозиторий · низкий · проверено
- C1-07 · гонки · app_state/src/lib.rs:462, 729 · open одного пути дважды / open ∥ close →
  два RepoId или вотчер-сирота · проверка и регистрация под одним локом · средний
- C1-08 · гонки · commands/mod.rs:2064, 277, 1824–1860, desktop.rs:80 · записи мимо очереди
  (chmod, config, hooks, trash) · через `mutating` · низкий · проверено
- C1-09 · гонки · app_state/src/queue.rs:83 · очередь по RepoId, а не по git-dir: worktree и
  основной репозиторий, сабмодуль и родитель пишут параллельно · ключ — common-dir ·
  средний · вероятно
- C1-10 · гонки · commands/mod.rs:1478 `repositories` · синхронная команда на воркере tokio
  с чтением head/branches/status · `blocking()` · низкий · проверено
- C1-11 · гонки · src-tauri/src/shutdown.rs:58 · `cancel_all()` на каждый CloseRequested до
  решения страницы → «Отмена» закрытия оставляет поиски отменёнными · отменять при
  фактическом выходе · низкий · проверено
- C1-12 · гонки · fs_watcher/src/watcher.rs:145 · тихое окно глушит и внешние события, не
  откладывая их → сохранение файла сразу после мутации теряется · копить подавленное и
  выдать по окончании окна · средний · проверено
- C1-13 · гонки · app_state/src/lib.rs:1309 `close_repository` · не ждёт очередь; запись
  Undoable с мёртвым id остаётся в журнале · в `record` проверять репозиторий · средний
- C1-14 · гонки · git_engine/src/children.rs:49 · хэндл закрыт раньше удаления из RUNNING →
  `taskkill` по переиспользованному PID · удалять до закрытия · низкий · вероятно
- C1-15 · гонки · git_engine/src/hooks.rs:291 · общий `COGIT_HOOK_MSG` у параллельных
  dry-run · уникальное имя · низкий · вероятно
- C1-16 · гонки · app_state/src/lib.rs:386, git_engine/src/discover.rs:92 · скан после
  закрытия диалога идёт до конца на общем пуле rayon · флаг остановки · низкий
- C1-17 · гонки · app_state/src/queue.rs:209 · Done уходит раньше `release` · сначала
  release · низкий · вероятно

`unsafe` (session_end, renderer_failure, webview2, recycle_bin): инварианты выписаны и
держатся — Box контекста subclass освобождается ровно раз в `WM_NCDESTROY`, вызовы COM —
внутри коллбеков на UI-потоке, PWSTR освобождаются `take_pwstr`, список путей для
`SHFileOperationW` с двойным NUL. Гарда `parking_lot` через `await` нет; разного порядка
захвата пар локов нет; `par_iter` только внутри `spawn_blocking`.

#### 1. Гонки — фронтенд, проверено как корректное

Проверено и корректно: `connect()`/`onMenuCommand` (отписка через `pending.then`),
поколения в `commit`/`worktree`/`stashView`/`compareView`/`commitTree`/`graph`,
content-search, таймеры Investigate/GraphCanvas/TooltipLayer.

#### 2. Баги и несостыковки

Проверено: 183 команды против биндингов и вызовов фронта (вызовов несуществующих команд
нет), 7 событий (все слушаются и эмитятся).

- B-01 · баг · app_state/src/safety.rs:98 · Undo для merge/rebase/cherry-pick/revert/
  interactive_rebase/split_off делает `git branch <текущая> <old>` → «already exists»,
  Undo всегда падает · для существующей ветки — `update-ref` с проверкой прежнего значения ·
  высокий · проверено
- B-02 · проглочено · app_state/src/lib.rs:621 `discard_paths` · `stash_paths(..)
  .unwrap_or(None)`: stash не удался → discard без резервной копии · отказать в discard ·
  высокий · проверено
- B-03 · проглочено · app_state/src/lib.rs:972 `rollback_to` · то же · пробрасывать Err ·
  высокий · проверено
- B-04 · проглочено · app_state/src/settings.rs:22 · битый/занятый settings.json читается
  как `{}`, `write_key` затирает остальные настройки · при ошибке чтения не писать · высокий
- B-05 · баг · git_engine/src/hooks.rs:519 `add_eol_rule` · не-UTF-8 `.gitattributes`
  заменяется одной строкой · читать байтами, дописывать `append` · высокий · проверено
- B-06 = C1-02
- B-07 · баг · git_engine/src/state.rs:57 · MERGE_HEAD проверяется раньше rebase-merge →
  конфликт на merge-коммите в `rebase -r` определяется как Merging · сначала rebase ·
  средний · проверено
- B-08 · баг · git_engine/src/hooks.rs:146 · хуки в linked worktree берутся из приватного
  git_dir, git — из common dir · `common_dir()` · средний
- B-09 · баг · fs_watcher/src/watcher.rs:57 · в linked worktree refs/packed-refs/config
  (common dir) не наблюдаются · common dir · средний
- B-10 · проглочено · fs_watcher/src/watcher.rs:39 · ошибки notify выбрасываются без лога ·
  warn · низкий
- B-11 · несостыковка · fs_watcher/src/watcher.rs:54 · корень рабочего дерева —
  `RecursiveMode::Recursive`, INV-06 запрещает; фильтр при маршрутизации · записать
  отступление в 01-architecture/12-risks (поведение не менять) · низкий
- B-12 · баг · git_engine line_history.rs:46, find.rs:158 · `log -L` без
  `core.quotepath=off`/фиксированных префиксов: Unicode-пути в кавычках, у пути с пробелом —
  хвостовой `\t`, `diff.noprefix` ломает срез · флаги + trim · средний
- B-13 · баг · git_engine/src/surgery.rs:171, :82 · `--name-only` без `-z` → Split Off
  отказывает для не-ASCII пути · `-z` · низкий
- B-14 · баг · git_engine/src/staging.rs:36 и др. · пути из UI как pathspec без
  `GIT_LITERAL_PATHSPECS` → discard `test[1].txt` задевает `test1.txt` · env · средний
- B-15 · баг · app_state/src/ref_ops.rs:30 · hard reset берёт `stashes().next()` без
  сравнения с вершиной до операции · сравнивать · средний · вероятно
- B-16 · баг · git_engine/src/repo.rs:264 · Undo удаления аннотированного тега создаёт
  лёгкий · хранить oid объекта тега · средний
- B-17 · несостыковка · app_state/src/ref_ops.rs:44, 79, 105; lib.rs:842 · reset mixed/soft,
  edit_author, rename_tag, abort, `rename_branch -M` без записи Recovery (INV-12) · средний
  — отложить (новое поведение Undo)
- B-18 · проглочено · app_state/src/lib.rs:1082 `flow_finish` · `rev-parse` через `.ok()` ·
  `--verify refs/heads/..^{commit}` · низкий
- B-19 · баг · git_engine/src/network.rs:117 · stderr декодируется кусками по 4096 байт →
  U+FFFD на границе многобайтного символа (INV-05) · копить байты · низкий
- B-20 · баг · runner.rs:324, output_text.rs:97 · редакция URL по первому `@` → часть
  пароля с `@` в журнале · последний `@` в authority · низкий
- B-21 · проглочено · git_engine/src/blobs.rs:160 · занятый файл показывается удалённым ·
  NotFound → None, прочее → Io · средний
- B-22 · баг · commands/mod.rs:348 `diagnostics` · `crate::webview2` есть только под
  `cfg(windows)` → не собирается на macOS/Linux · команда мертва (D3-05) — удалить · низкий
- B-23 = D3-05
- B-24 · проглочено · git_engine/src/surgery.rs:116 `recover` · `let _ =` на восстановлении ·
  добавлять ошибки восстановления в ответ · низкий
- B-25 · проглочено · git_engine/src/stash.rs:76 · повтор без `--index` на любую ошибку ·
  низкий · вероятно
- B-26 · баг · git_engine/src/state.rs:66 · многокоммитный cherry-pick после ручного Commit
  → Clean, баннер пропадает · учитывать `sequencer/todo` · низкий · вероятно
- B-27…B-30 · док-устарел · 04-ipc-contract §3, §4, §6; 03-git-semantics §3 · привести к
  коду · низкий
- B-31 · несостыковка · git_engine/src/network.rs:193 · токен в `-c http.extraHeader` виден
  в командной строке и уходит на все хосты вызова · средний · **на решение**

#### 3. Неиспользуемое

Чисто: ни одного `#[allow(dead_code)]`/`#[allow(unused…)]`, нет `[features]`, каждая
зависимость каждого крейта используется (по grep; `cargo machete` не установлен), `git`
запускается только через `git_engine::runner`.

- D3-01 · app_state/src/lib.rs:288 `with_secrets` · 0 вызовов · удалить · низкий
- D3-02 · diff_engine/src/lib.rs:189 `DiffError` · 0 ссылок; единственный пользователь
  `thiserror` в крейте · удалить enum и зависимость · низкий
- D3-03 · app_state/src/lib.rs:1629 `SharedState` · 0 · удалить · низкий
- D3-04 · fs_watcher/src/watcher.rs:90 `pause`/`resume` · только тесты, в продукте их
  заменили quiet-окна · удалить (тесты этих методов уходят вместе с ними) · низкий
- D3-05 · commands/mod.rs:1458, 1501, 171 · команды `reflog`, `submodules`,
  `list_all_repo_files` (+ обёртки в app_state) фронт не вызывает; плюс `diagnostics`
  (S5-13) · удалить, перегенерировать биндинги, 04-ipc-contract · низкий
- D3-06 · git_engine/src/state.rs:41 `allows_commit` · только юнит-тест · удалить · низкий
- D3-07 · search.rs:71 `stream_commits`, app_state lib.rs:489 `stream_graph` · только
  тесты · тестам звать `search_*` · низкий
- D3-08 = F3-07 · lib/settings.ts:26 `gitPath` · настройка сохраняется, никто не читает ·
  **на решение пользователя** (подключить в runner или убрать из панели) · средний
- D3-09 · Cargo.toml:21 · фичи tokio `process`, `fs`, `io-util` не используются · убрать ·
  низкий (tauri может включать их сам — выигрыша в сборке может не быть)
- F3-01 · components/common/Tooltip.svelte · не импортируется; Toolbar ставит `data-tip`
  руками · перевести Toolbar на него или удалить · низкий
- F3-02 · lib/links.ts · только свой тест · удалить с тестом · низкий
- F3-03 · lib/ipc/index.ts:321, 355, 921, 940 · `deleteUntracked`, `setUpstream`,
  `diffFiles`, `fileBefore` и реэкспорт типов :931 — 0 вызовов · удалить · низкий
- F3-04 · lib/investigate/blame.ts:16 `sourceOf`, params.ts:54 `sameLocation` · 0 ·
  удалить · низкий
- F3-05 · lib/settings-file.ts:32 `forgetSettings` · 0 · удалить · низкий
- F3-06 · экспорты только для своих тестов: availability `NOTHING`/`reasons`, selection
  `hunkSelection`/`selectedRange`, preferences `changedKeys`/`sameKeymap`, tri-state-box
  `faceOf`, toolbar `NO_FACTS`, repo-list `EMPTY_LIST`, graph-geometry `toListRow`,
  file-view `TOGGLES` · проверить по одному; удалять только функции, которые не нужны
  продукту (тесты удаляемой функции уходят вместе с ней) · низкий
- F3-08 · app.css:394–435 · 9 селекторов `.tok-*`, которых `classHighlighter` не выдаёт ·
  удалить · низкий

#### 4. Дубли

- D4-01 · diff_engine/src/images.rs:38 + git_engine/src/network.rs:156 · две копии `base64`
  · одна копия (новая зависимость не нужна) · низкий
- D4-02 · git_engine find.rs:220 + line_history.rs:166 · одинаковые `path_of` и разбор
  заголовка `git log -L` · общий модуль · низкий
- D4-03 · **баг** · git_engine find.rs:157, line_history.rs:45, interactive.rs:95,
  surgery.rs:165, 200 · разбирают `run_git_reading(..).stdout` — вывод для журнала, который
  `output_text::trim` режет на длинном выводе; для разбора есть `read_git` (R-280) ·
  перевести на `read_git` · низкий
- D4-04 · **баг** · git_engine/src/apply.rs:36 · свой spawn мимо runner: команда без
  `redact_command`, stdin пишется в том же потоке (риск взаимной блокировки на большом
  патче) · `run_git_fed` · низкий
- D4-05 · runner.rs:284 `base_command` + :359 `bare_git` · общее ядро настройки · низкий
- D4-06 · hooks.rs:334 + :393 · почти одинаковые `bash`-команды, литерал `0x0800_0000`
  вместо `CREATE_NO_WINDOW` (третья копия в app_state/desktop.rs:315) · один хелпер · низкий
- D4-07 · app_state terminal.rs:67 + desktop.rs:160 · две абстракции запуска терминала;
  `open_in_terminal` зовёт `Command::new` сам · средний (меняет запуск терминала) — отложить
- D4-08 · commands/mod.rs:1590, 2017, 2350, investigate.rs:143 · 4 одинаковых открытия
  дочернего окна · общий хелпер · низкий
- D4-09 · git_engine: 6 копий `u32::try_from(elapsed.as_millis()).unwrap_or(MAX)` ·
  `elapsed_ms` · низкий
- D4-10 · 9 ручных `replace('\\', "/")` для IPC-путей · `ipc_path` в git_engine · низкий
- D4-11 · commands/desktop.rs:85, mod.rs:1786, 1798 · ручные `map_err` при наличии `From` ·
  `From` · низкий
- D4-12 · app_state/src/settings.rs:9, 14 · `path` = обёртка над `file_in` · одна · низкий
- F4-01 = F3-01
- F4-02 · SplitOffDialog, RebaseEditor, HooksPanel, SafetyJournal · свои модалки вместо
  `Dialog.svelte` · средний (видимое поведение Enter/фокус) — отложить
- F4-03 · 5 своих всплывающих меню в тулбарах; `Esc` закрывает только два · общий
  `PopupMenu` · средний — отложить (это новая подсистема UI)
- F4-04 · App.svelte:2248, 2396, 2962 · контекстные меню литералами мимо `item()`/`tidy()`
  · в lib, с тестом · низкий
- F4-05 · 3 вызова `revealItemInDir` мимо `fileMenus.revealOnDesktop` · низкий
- F4-06 · 4–6 копий копирования в буфер · `lib/clipboard.ts` · низкий
- F4-07 · ~27 мест `.slice(0, 7)` вместо `shortOid` · низкий
- F4-08 · 6 функций «последний сегмент пути» · одна `baseName` · низкий
- F4-09 · App.svelte:2380 + repo-click.ts:19 · `samePath` дважды · низкий
- F4-10 · lib/investigate/blame.ts:43 `shortAuthor` режет строку сам · низкий
- F4-11 · нативные checkbox/select в обход `Checkbox`/`Select` · средний (видимый UI) —
  отложить
- F4-12 · подписи сочетаний клавиш зашиты строками, после переназначения врут · средний —
  баг, на решение (затрагивает много UI)

#### 5. Упрощение — фронтенд

- F5-01 · App.svelte:1757 · ветки `merge` и `fastForward` одинаковы · объединить (или это
  баг ff-only — проверить) · низкий
- F5-02 · App.svelte:2455 · повтор `isActive()` · низкий
- F5-03 · App.svelte:508–841 · палитра команд — `$derived.by` на 333 строки · вынести в
  `lib/palette-commands.ts` · средний

#### 5. Упрощение — бэкенд

Горячий путь меняется только с A/B (правило проекта); ночью A/B на каждый пункт не
уложить — берутся только правки без изменения API движков.

- S5-01 · упрощение · app_state/src/lib.rs:524 `search_graph` · копии `oid`/`parents` на
  коммит ради `CommitNode` · заимствования в `CommitNode<'a>` · средний (API graph_engine) ·
  измеримо
- S5-02 · упрощение · git_engine/src/history.rs:55 `row_of` · заголовок коммита разбирается
  трижды · один проход `commit.iter()` · низкий · вероятно измеримо
- S5-03 · упрощение · git_engine/src/search.rs:245, :56 · lowercase иглы на каждом коммите
  в фильтре · готовить иглы один раз · низкий · измеримо в фильтре
- S5-04 · упрощение · git_engine/src/search.rs:196 `shown_by` · полный `row_of` родителя ·
  кеш на загрузку · средний · измеримо в фильтре
- S5-05 · упрощение · search.rs:149, topo.rs:30 · Vec родителей на коммит · обобщить
  `Ordered` · низкий · слабо
- S5-06 · упрощение · graph_engine/src/lanes.rs:53, :76 · два одинаковых поиска · один ·
  низкий · нет
- S5-07 · упрощение · app_state/src/graph_cache.rs:118 · копия 128 строк ради `encode` ·
  кодировать из среза · низкий · нет
- S5-08 · упрощение · diff_engine/src/batch.rs:29 · `from_utf8_lossy` для бинарных файлов ·
  только для `FileDiff::Text` · низкий · измеримо на бинарных
- S5-09…S5-11 · упрощение · diff_engine text.rs:251, moves.rs:19, text.rs:180 · лишние копии
  строк · `into_iter`, `&str` · низкий · слабо
- S5-12 · упрощение · git_engine/src/origin_search.rs:104 · `Found.source` — копия файла на
  каждое совпадение · `Rc<[String]>` · низкий · измеримо на крупных файлах
- S5-13 · неиспользуемое · commands/mod.rs:1501, :171, :1458, :344 · команды `submodules`,
  `list_all_repo_files`, `reflog`, `diagnostics` фронт не вызывает · удалить (см. кат. 3)
- S5-14 · неиспользуемое · search.rs:71 `stream_commits`, app_state lib.rs:489
  `stream_graph` · вызывают только тесты · тестам звать `search_*` · низкий
- S5-15 · баг · commands/mod.rs:2441 `declared()` · список файлов для проверки main-thread
  без `toolbar.rs`; макросные команды не видны · строить из фактического списка · низкий
- S5-16 · упрощение · git_engine/src/commit.rs:198 `to_entry` · 4 повторные ветки · общий
  хелпер · низкий

Длинные функции: `place` (lanes.rs:20, 197 строк, 4 шага), `origin_candidates`
(origin_search.rs:114, 155, 3 фазы), `diff_engine::text::build` (106, повтор цикла контекста),
`to_entry` (102), `run` (lib.rs:306, 97), `merge3_with_syntax` (89). Не делятся:
`specta_builder`, `graph_wire::encode`.

#### 6. Модули

- M6-01 · модули · src-tauri/src/commands/mod.rs (2612) · по файлам меню/панелей:
  worktrees, stash, rewrite, hooks, conflicts, diff, blame, flow, presets, avatars, app;
  ref_ops/remote_ops/desktop/investigate уже есть — дособрать; тесты в `tests.rs` · чистый
  перенос, сначала S5-15 · низкий
- M6-02 · модули · app_state/src/lib.rs (1814) · registry, journal, stashing, rewrite, flow;
  дособрать в существующие graph_cache, ref_ops, investigation, diffing · низкий
- M6-03 · модули · src-tauri/src/menu.rs (956) · `menu/keymap.rs`, `menu/context.rs`,
  тесты · низкий
- M6-04 · модули · test_fixtures/src/lib.rs (846) · `import.rs` (fast-import) · польза
  мала, крейт тестовый · не делать

## Фаза 2 — исправление

Каждый баг — `fix(...)` с тестом, который его воспроизводит. Отложенное — в итоге.

### Категория 1

- [x] C1F-01 `refresh` во время `opening`
  > Итог: `refresh` выходит, пока идёт `opening` — открытие в полёте и так принесёт свежее;
  > тест `does not let a refresh during an open take the user back`.
- [x] C1F-08 перебитый `activate` выходит
  > Итог: `open` возвращает, выиграл ли его ticket; перебитый `activate` выходит. Тест
  > `tells the caller whose open was overtaken`.
- [x] C1F-02/03/04 проверка «репозиторий всё ещё текущий» после await в App
  > Итог: `repository.epoch` меняется только при переходе на другой репозиторий (не при
  > перечитывании); `applyDiskChanges`, `mutate`, `commitStaged`, `afterRefChange`,
  > `runNetwork` после каждого await выходят, если эпоха сменилась. Тесты на эпоху — в
  > repository.test.ts; сама проводка в App.svelte юнит-тестами не покрыта (правило фронта:
  > тестируем сторы, не разметку).
- [x] C1F-05/06/07/14 поколения в сторах stashes, recovery, network, flow, conflicts,
  worktrees, submodules, refs
  > Итог: в stashes, recovery, network, flow, conflicts (список), worktrees, refs (даты,
  > URL) — счётчик: пишет только последний запрос, `clear()` гасит летящие; submodules —
  > поколение на смену владельца; `refreshProgress`/`loadTemplate` в App — по эпохе. Тесты
  > `leaving-repository.test.ts` (9, все падали до правки).
- [x] C1F-09 картинки диффа по поколению
  > Итог: картинки пишутся только при текущем поколении; тест `never shows the first image's
  > pictures under the second`.
- [x] C1F-10 пропуск blame увеличивает поколение
  > Итог: пропуск загрузки (на экране уже нужный blame) увеличивает поколение и снимает
  > loading; тест `keeps the blame of the place it went back to`.
- [x] C1F-12 тело хука по имени
  > Итог: тело пишется, только если редактор всё ещё на этом хуке; тест `hooks.test.ts`.
- [x] C1F-13 `refreshList` по ticket
  > Итог: пишет и запоминает в сессии только ответ последнего запроса; тест `keeps the newer
  > answer when an older one arrives last`.
- [x] C1F-15 PageDown по счётчику
  > Итог: не сделано: правка только в разметке CommitList (счётчик нажатий), тест возможен
  > лишь по разметке — правило фронта; риск низкий, оставлено.
- [x] C1F-17 выбор коммита сбрасывает stash
  > Итог: `commit.onchange` (уже сбрасывает дифф и сравнение) теперь сбрасывает и
  > `stashView`; `commit.clear()` onchange не вызывает, поэтому показ stash из References не
  > задет. **Без юнит-теста** — проводка App.svelte; на решение: оставить или откатить.
- [x] C1-02 форвардер событий переживает `Lagged`
  > Итог: `app_state::next_event`: `Lagged` → warn и дальше, `Closed` → конец; форвардер в
  > src-tauri читает через него. Тесты
  > `a_forwarding_loop_keeps_going_after_it_falls_behind`, `…_ends_when_the_bus_is_gone`.
- [x] C1-03 выключение аватаров без join под локом
  > Итог: сервис вынимается под локом (`take`/`replace`), дропается после; команда
  > выключения — через `blocking`. Тест `turning_avatars_off_does_not_stall_the_readers` (до
  > правки читатель ждал 1,8 с).
- [x] C1-04/05 индекс аватаров: запись по очереди и атомарно, IO вне лока
  > Итог: не сделано: C1-04 не воспроизводится (8 потоков × 100 параллельных сохранений
  > индекса, 3 прогона — ничего не потеряно), а без воспроизводящего теста баг-фикс не
  > принимается; ущерб в худшем случае — повторная загрузка аватара. C1-05 — это конкуренция
  > за лок, то есть скорость: по правилу проекта только с A/B.
- [x] C1-06 `row_for` не кладёт устаревшее
  > Итог: кэш строк — `RowCache` со счётчиком забываний: строка, прочитанная, пока
  > репозиторий менялся, не кладётся. Тесты `a_row_read_across_a_change_is_not_kept` (логика
  > кэша; сама гонка с вотчером детерминированно не воспроизводится) и
  > `a_row_read_undisturbed_is_kept`.
- [x] C1-07 открытие одного пути — атомарно
  > Итог: `find_or_register` ищет и регистрирует под одним `repos.write()`; вотчер
  > вставляется, только если записи ещё нет и репозиторий зарегистрирован;
  > `close_repository` после unregister снимает вотчер ещё раз (ловит поставленный
  > параллельным open). Тест `one_path_opened_twice_at_once_is_one_repository` (до правки —
  > два id, стабильно). Попутно: `close_repository` шлёт `RepoClosed` дважды (unregister +
  > сам) — в webview не пересылается, не трогал.
- [x] C1-08 записи через очередь
  > Итог: stage_mode (Stage), write_git_config (Other, если есть репозиторий; глобальный —
  > как было), write_hook/set_hook_enabled/use_hooks_path (Other), move_to_trash (Discard) —
  > через `mutating`. Видимое следствие: эти действия появляются в индикаторе очереди. Тест
  > `commands_that_write_the_repository_wait_for_its_lane` (сканирует тела команд, как
  > соседние тесты модуля).
- [x] C1-10 `repositories` в blocking
  > Итог: `repositories` — `async fn` + `blocking`; тип стал `Result<Vec<RepoOverview>>`
  > (async-команда с `State` обязана возвращать Result), `listRepositories` разворачивает
  > его; 04-ipc-contract обновлён. Тест `the_repository_list_is_read_off_the_async_workers`.
  > Попутно (в плане, раздел «Режим»): сбой со stash — см. ниже.
- [x] C1-11 отмена поисков только при фактическом выходе
  > Итог: отмена чтений перенесена из `shutdown::watch` (CloseRequested) в
  > `shutdown::exiting` на `RunEvent::ExitRequested` — его дают и согласие страницы, и
  > `app.exit` сторожа. Тест `a_close_request_leaves_the_running_reads_alone` (по исходнику,
  > как соседние тесты команд).
- [x] C1-12 тихое окно вотчера не теряет внешние события
  > Итог: не сделано, **на решение**: во время окна нельзя отличить свою запись от чужой, а
  > выдача подавленного по окончании окна возвращает двойную перезагрузку после каждой
  > мутации, ради устранения которой окно и введено (R-25, R-197). Внешнее сохранение в эти
  > ~400 мс видно только со следующим событием.
- [x] C1-13 журнал безопасности не держит закрытый репозиторий
  > Итог: `record` кладёт запись, только если репозиторий ещё зарегистрирован (проверка под
  > локом журнала); `close_repository` чистит журнал после unregister. Тест
  > `a_mutation_finishing_after_its_repository_closed_leaves_no_undo_entry`. Ожидание
  > очереди при закрытии (мутации в очереди падают с RepoNotFound) — не трогал: это
  > изменение поведения закрытия, **на решение**.
- [x] C1-15 уникальный файл сообщения для dry-run хука
  > Итог: имя файла сообщения — `COGIT_HOOK_MSG-<pid>-<n>`. Тест
  > `two_dry_runs_at_once_keep_their_own_message` (до правки второй прогон падал с exit 4).
- [x] C1-16 скан останавливается с закрытием канала
  > Итог: `discover::scan_until`: `false` из колбэка ставит флаг, обход перестаёт спускаться
  > и ничего больше не сообщает; `scan` — обёртка над ним; `scan_for_repositories` в
  > app_state — через `scan_until`. Тест `a_scan_told_to_stop_reports_nothing_more`.
- [x] C1-17 release раньше Done
  > Итог: не сделано: окно между emit и release — микросекунды, детерминированного теста
  > нет. Правка — поменять две строки в `settle` местами (release, потом emit); порядок
  > событий разных операций фронту не важен (`applyPending` — словарь по id). **На
  > решение.**

### Категория 2

- [x] B-01 Undo merge/rebase двигает существующую ветку
  > Итог: новый `Recovery::Moved` для merge, rebase, interactive rebase, split off, cherry-
  > pick, revert; Undo зовёт `move_branch_back`: текущая ветка — `reset --keep` (не затирает
  > локальные правки), другая — `branch --force`. `Recovery::Branch` остался для удалённых
  > веток. Тесты `undoing_a_merge_puts_the_branch_back_where_it_was` (до правки — «already
  > exists»), `…_after_the_user_left_it`.
- [x] B-02/B-03 discard и rollback не идут без резервного stash
  > Итог: ошибка резервного stash прерывает discard и rollback до изменения файлов
  > (`backup_failed`, с логом); R-284 в 12-risks — отступление: огромное выделение теперь
  > отказывает, а не выбрасывается без Undo (**на решение**, если нужно иначе — например,
  > stash порциями). Тест `a_discard_whose_backup_fails_throws_nothing_away` (пустой
  > репозиторий: до правки файл удалялся).
- [x] B-04 settings.json: ошибка чтения не затирает файл
  > Итог: `write_key`: файл, который не читается (не NotFound), — ошибка без записи;
  > непарсящийся — сохраняется как `settings.json.damaged` и заменяется (существующий тест
  > замены сохранён); `read_document` логирует ошибку чтения. Тест
  > `a_damaged_file_is_kept_aside_before_the_next_write_replaces_it`.
- [x] B-05 `.gitattributes`: дописывать, не перезаписывать
  > Итог: `.gitattributes` читается байтами, правило дописывается через `append`, ошибка
  > чтения, кроме NotFound, пробрасывается. Тест
  > `a_gitattributes_that_is_not_utf8_keeps_its_bytes` (cp1251-комментарий: до правки файл
  > заменялся одной строкой).
- [x] B-07 rebase раньше merge в определении состояния
  > Итог: rebase-merge/rebase-apply проверяются раньше MERGE_HEAD, как в wt-status.c. Тест
  > `a_merge_stopped_inside_a_rebase_is_a_rebase`.
- [x] B-08/B-09 хуки и вотчер worktree — common dir
  > Итог: `RepoHandle::common_dir` (gix `common_dir`, сверено с исходником 0.87.1); хуки без
  > `core.hooksPath` — из common dir; вотчер получил `common_dir`: в linked worktree
  > дополнительно наблюдает `<common>/refs` и верх common dir (packed-refs, config),
  > маршрутизация — сначала приватный каталог, потом общий. Тесты
  > `a_linked_worktree_lists_the_hooks_git_runs_there`,
  > `a_linked_worktree_hears_about_refs_in_the_common_directory`.
- [x] B-10 ошибки notify в лог
  > Итог: (fs_watcher получил `tracing.workspace = true` — крейт уже в сборке, нового ничего; правило «каждый fallback логирует Err») ошибка notify — `tracing::warn!` ("the file watcher lost events"); полный Refresh
  > на ошибку не добавлял — это новое поведение. Без теста: меняется только лог.
- [x] B-12 `log -L`: quotepath, префиксы, таб
  > Итог: уже работало: git 2.51 в `log -L` не экранирует не-ASCII пути даже при
  > `core.quotepath=true`, не дописывает таб после имени с пробелом и не применяет
  > `diff.noprefix` (проверено вручную и тестом, в том числе для папки `b/`). Кода не менял;
  > добавил тест-страховку `the_path_of_each_version_is_the_file_itself_whatever_its_name`.
- [x] B-13 surgery: `-z`
  > Итог: `diff-tree` и `ls-tree` в split off — с `-z`, разбор по `\0`. Тест
  > `a_file_with_a_non_ascii_name_can_be_split_off` (при `core.quotepath` по умолчанию до
  > правки — «not one of the files this commit changed»). Остальные списки путей в
  > git_engine уже с `-z` или не разбирают путь.
- [x] B-14 литеральные pathspec
  > Итог: `run_git_paths` (stage, unstage, discard, clean, stash, restore для rollback)
  > ставит `GIT_LITERAL_PATHSPECS=1` в обеих ветках (аргументы и stdin). Тест
  > `discarding_a_name_with_brackets_touches_only_that_file` (до правки `clean` удалял и
  > `test1.txt`); `staging_a_name_with_brackets_…` — страховка (git add и так брал точное
  > совпадение).
- [x] B-15 hard reset: свой stash по сравнению вершин
  > Итог: сценарий аудитора не происходит (`stash_push` уже сравнивал вершины), но тест
  > нашёл другой баг: при сдвинутом только сабмодуле hard reset отказывал с «there is
  > nothing to stash», хотя `reset --hard` сабмодуль не трогает. Новый `stash_push_if_any` →
  > `Option<oid>`; reset без копии идёт дальше, `stashes().next()` убран. Тест
  > `a_hard_reset_with_only_a_moved_submodule_goes_ahead_without_a_backup`.
- [x] B-16 Undo удаления аннотированного тега
  > Итог: в журнал пишется прямая цель `refs/tags/<name>` (`RepoHandle::tag_target`, gix) —
  > у аннотированного это объект тега; `git tag <name> <tag-object>` восстанавливает ссылку
  > на него же. Тест `undoing_the_deletion_of_an_annotated_tag_brings_the_annotation_back`.
- [x] B-18 `flow_finish` rev-parse
  > Итог: oid ветки берётся из списка веток (как в `delete_branch`), а не `rev-parse <имя>`,
  > который предпочитает одноимённый тег; ошибка чтения веток теперь пробрасывается, а не
  > глотается. Тест `undoing_a_finished_feature_restores_the_branch_not_a_tag_of_that_name`.
- [x] B-19 stderr сети целиком
  > Итог: stderr копится байтами (`Progress`), строка декодируется целиком после `\r`/`\n`,
  > полный текст — один раз в конце. Тесты
  > `a_character_split_between_two_reads_arrives_whole` (разрез посреди кириллической
  > буквы), `carriage_returns_split_progress_and_the_tail_is_delivered`.
- [x] B-20 редакция URL по последнему `@`
  > Итог: в `redact_url` (команда) и `redact_urls` (вывод git) authority кончается на первом
  > `/`, учётные данные отделяются по последнему `@`. Тесты
  > `a_password_with_an_at_sign_is_hidden_whole` и `…_in_git_output_too`.
- [x] B-21 `blob_on_disk`: только NotFound — «нет файла»
  > Итог: `blob_on_disk` → `Result<Option>`: нет файла или это каталог (сабмодуль) — `None`,
  > иначе `GitError::Io`; три вызова пробрасывают ошибку. Тесты
  > `a_working_file_that_cannot_be_read_is_an_error_not_a_deletion` (Windows, файл открыт
  > без общего доступа) и страховка
  > `a_directory_on_the_working_side_still_reads_as_nothing`.
- [x] B-24 ошибки `recover`
  > Итог: `checkout --force` и `reset --hard` при откате неудачного split —
  > `tracing::error!` на сбой (шаги и так видны в журнале Output); `rebase --abort` без
  > rebase падает законно — оставлен без лога. Без теста: меняется только лог.
- [ ] B-26 sequencer/todo
- [ ] D4-03 разбор через `read_git`
- [ ] D4-04 apply через runner
- [ ] S5-15 `declared()` видит все команды
- [ ] B-11, B-27…B-30 документация

### Категории 3–6

- [ ] D3-01…07, D3-09, B-22 мёртвый Rust
- [ ] F3-01…06, F3-08 мёртвый фронт
- [ ] D4-01, 02, 05, 06, 08…12 дубли Rust
- [ ] F4-04…10 дубли фронта
- [ ] F5-01, F5-02, S5-06, S5-16 упрощение
- [ ] M6-03 menu.rs (перенос)
- [ ] M6-02 app_state/lib.rs (перенос)
- [ ] M6-01 commands/mod.rs (перенос)

## Фаза 3 — итог

- [ ] Полный прогон тестов и бенчмарк, сравнение с фазой 0
- [ ] Итоговые таблицы
