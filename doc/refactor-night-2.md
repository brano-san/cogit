# Рефакторинг, вторая итерация, 2026-09-24

Источник истины для автономной работы. После сжатия контекста или авто-продолжения —
сначала перечитать этот файл, потом продолжать со следующего неотмеченного шага.
Первая итерация — `doc/refactor-night.md`; её находки здесь не повторяются.

## Задание (сжато)

Та же цель и те же правила, что в первой итерации.

- Без push, force и переписывания истории. Без вопросов: спорное решаю сам, одной строкой здесь.
- Шаг: правка → затронутые тесты → clippy → коммит (этот файл — в том же коммите) → отметка
  `[x]` и строка `> Итог:`. Три неудачные попытки → `не сделано: причина`, дальше.
- Поведение не меняется. Найденный баг — отдельный коммит `fix(...)` с тестом, который его
  воспроизводит. Тесты не удалять и не ослаблять; флап тестов с замером времени — не
  регрессия, отметить здесь.
- Фаза 0 — полный прогон и бенчмарк. Фаза 1 — аудит (≤ ~15 % работы), категории: 1 гонки,
  2 баги/несостыковки, 3 неиспользуемое, 4 дубли, 5 упрощение, 6 модули. Фаза 2 —
  исправление в порядке 1 → 6; перенос кода — отдельным коммитом без логики; у правок 3–6 —
  польза одной строкой, без пользы не делать, без переименований по вкусу, без новых
  зависимостей. Фаза 3 — прогон и бенчмарк, сравнение, регрессия скорости → найти коммит и
  откатить; итоговые таблицы.

## Режим

- Ветка `refactor/night-2` от `refactor/night` b6e89a6: первая итерация ещё не слита, вторая
  строится поверх неё; слить можно любую из двух, вторая включает первую.
- Тяжёлое — на ядрах 16–31: `cmd //c <scratchpad>\aff.cmd <команда>`; бенчмарк —
  `--hidden --cores 16-31`.
- Коммиты — одна строка без трейлеров.
- Падение теста без правки показываю порядком TDD (тест → красный прогон → правка), stash не
  трогаю (урок первой итерации).
- В сессии стоит cron (:23 и :53) «перечитай план и продолжай» на случай лимита; удалить,
  когда фаза 3 отмечена.
- Решения первой итерации, оставленные пользователю (14 пунктов в конце
  `doc/refactor-night.md`), здесь не решаю и не трогаю.

## Фаза 0 — базовая точка

- [x] Полный прогон тестов до правок
  > Итог: nextest 1678 ✓ / 1 skipped (70 с); `cargo test -p cogit --lib` 103 ✓; vitest 1671
  > ✓ (123 файла); svelte-check 0/0; clippy `-D warnings` чисто — совпадает с итогом первой
  > итерации.
- [x] Бенчмарк до правок
  Решение: код b6e89a6 совпадает с 93b9bc4 (после него — только документы), а
  `target/bench/exes/night-final.exe` собран из 93b9bc4 и уже сравнён с `night-base` (все 85
  сценариев «same»). Он и есть база; новый полный прогон не нужен — в фазе 3 A/B
  `night-final` против итоговой сборки в одной сессии, как в первой итерации.
  > Итог: база — `night-final.exe` (см. решение выше), отдельный прогон не делал.

## Фаза 1 — аудит

Формат: `id · категория · файл:строка · что не так · как исправить · риск`. Номер — `<область>-<категория>-<nn>`: GE git_engine, AS app_state и малые крейты,
ST src-tauri, FS сторы и lib фронта, FA App.svelte/окна/layout, FC остальные компоненты.

- [x] Аудит по шести категориям
  > Итог: шесть агентов только на чтение, по областям (а не по категориям, как в первой
  > итерации — другой срез даёт новое); 196 находок, из них 7 дублей между агентами. Каждую
  > проверяю по коду перед правкой.

### Находки

#### git_engine (GE)

- GE-1-01 · гонки · crates/git_engine/src/stash_rename.rs:24-45 · drop…store без блокировки; падение store посреди цикла теряет записи после неё · собрать oid заранее, при ошибке дописать остальные · низкий · вероятно
- GE-2-01 · проглочено · worktree.rs:238 add_to_gitignore · read_to_string().unwrap_or_default() → не-UTF-8/занятый .gitignore перезаписывается одной строкой (класс B-05) · fs::read, NotFound=пусто, append · высокий · проверено
- GE-2-02 · баг · worktree.rs:242-248 · имя в .gitignore без экранирования: test[1].txt игнорирует test1.txt, #x/!x, без ведущего / · экранировать, ставить / · средний · проверено
- GE-2-03 · баг · staging.rs:62,69-72; conflicts.rs:97; commit_write.rs:34-36; file_log.rs:67; file_ops.rs:67,203/217 · остаток B-14: пути как glob; stage_mode пишет blob test1.txt в test[1].txt · GIT_LITERAL_PATHSPECS везде; stage_mode — oid из gix-индекса · высокий · проверено
- GE-2-04 · баг · commit_write.rs:143-147 · commit --only кладёт пути в командную строку (R-191) → os error 206 на тысячах · run_git_paths / pathspec-from-file · средний · проверено
- GE-2-05 · баг · file_ops.rs:203 apply_commit_file · porcelain git diff читает diff.noprefix/color.ui/diff.external → apply падает · diff-tree -p или --no-color --no-ext-diff --src/dst-prefix · средний · проверено
- GE-2-06 · баг · interactive.rs:78-81 · reword кладёт многострочное сообщение в exec одной строкой → todo невалиден, repo в незавершённом rebase · сообщение в файл, commit --amend -F · высокий · проверено
- GE-2-07 · баг · interactive.rs:77,100-108 · rebase_todo даёт в message только %s: Reword в RebaseEditor стирает body, Squash заменяет объединённое сообщение subject'ом · exec только при явном сообщении, subject отдельным полем · высокий · проверено
- GE-2-08 · баг · interactive.rs:100, edit_author :139 · log --reverse base..HEAD включает merge → pick <merge> падает, rebase остаётся посреди · --no-merges --topo-order или отказ · средний · проверено
- GE-2-09 · баг · surgery.rs:72 · rebase --onto без --rebase-merges выпрямляет merge после target в split_off · --rebase-merges или отказ · средний · проверено
- GE-2-10 · баг · surgery.rs:85,179,222 · остаток D4-03: -z вывод проходит normalise/redact (token/secret/password в имени → ***), for-each-ref --contains >20k обрезается · read_git · средний · проверено
- GE-2-11 · несостыковка · flow.rs:225-233, file_ops.rs:66-68, staging.rs:62, stash.rs:75-76 · проверки с ожидаемым отказом идут через журнал → ложные уведомления об ошибке (flow_start каждый раз) · gix или children::output мимо журнала · средний · проверено
- GE-2-12 · баг · history.rs:22-46 (reflog.rs:141) · graph_tips без тегов → Lost Commits показывает коммиты под тегом · добавить теги · средний · проверено
- GE-2-13 · несостыковка · reflog.rs:117 · lost commits только по reflog HEAD, T5.6 обещает logs/refs · обходить logs/refs · средний · проверено
- GE-2-14 · баг · reflog.rs:157 · инкрементальный Reachable падает на отсутствующем родителе (shallow), кэш остаётся → каждый вызов падает · считать листом · низкий · вероятно
- GE-2-15 · баг · worktrees.rs:158-175 main_root · linked worktree bare-репозитория: текущий показан дважды, один раз как main · common dir bare = main · средний · проверено
- GE-2-16 · баг · hooks.rs:130-148, 492-505 · core.hooksPath/commit.template через .string() без раскрытия ~ → template None, write_hook создаёт папку ~ · trusted_path · средний · проверено
- GE-2-17 · несостыковка · hooks.rs:339-364, 405-418 · dry run/run_check наследуют GIT_DIR/GIT_WORK_TREE… (R-22); bash по PATH может быть WSL · env_remove(INHERITED_GIT_VARS); bash рядом с git · средний · env проверено, bash вероятно
- GE-2-18 · баг · flow.rs:197-201 · конфликт merge в develop после создания тега → повторный finish падает «tag already exists» · не создавать тег, если уже на вершине main · средний · проверено
- GE-2-19 · баг · tags.rs:81-91 · rename_tag пересоздаёт аннотацию с cleanup=strip → строки с # теряются · --cleanup=verbatim · низкий · проверено
- GE-2-20 · баг · progress.rs:87-98 · done считает exec/break, todo их отбрасывает → 2/4 вместо 1/3 · тот же фильтр · низкий · проверено
- GE-2-21 · несостыковка · commit.rs:265-273 · процент rename своей формулой, не stats.similarity gix · similarity*100 · низкий · проверено
- GE-2-22 · несостыковка · network.rs:89-138 · doc/03 §3.6 требует таймаут и отмену сети, нет ни того, ни другого, нет в 12-risks · отмена/lowSpeed или запись в risks · средний · проверено
- GE-2-23 · несостыковка · network.rs:94-98 · stream_git наследует stdin · Stdio::null · низкий · вероятно
- GE-2-24 · баг · runner.rs:99-117 · run_git_bytes при ошибке не пишет запись в журнал, id в ошибке ведёт в никуда · journal_entry + elapsed_ms · низкий · проверено
- GE-2-25 · баг · reflog.rs:80-90 + stash.rs:93-104 · нумерация stash после пропуска битых строк reflog расходится с stash@{n} · позиция до фильтрации · низкий · проверено
- GE-2-26 · баг · phases.rs:51 · литерал «using up to 8 threads» · starts_with · низкий · проверено
- GE-3-01 · неиспользуемое · error.rs:59 GitError::RepoBusy · не создаётся; фронт lib/ipc/index.ts:161 мёртвый case; doc/04 обещает · удалить или создавать при index.lock · польза: контракт честный
- GE-3-02 · неиспользуемое · status.rs:23 RepoStatus::total() · 0 вызовов · удалить · польза: меньше API
- GE-3-03 · неиспользуемое · state.rs:28 is_interrupted_operation · только свои тесты (как D3-06) · вместе с D3-06
- GE-4-01 · дубль · ancestry.rs:55-66 = reset.rs:61-72 · commit_id = resolve_commit · оставить одно · польза: одно место
- GE-4-02 · дубль · staging.rs:50 = file_ops.rs:24 (+ branches/tags/subtrees) · require_paths/require · один хелпер · польза: одна формулировка
- GE-4-03 · дубль · surgery.rs:127-133, reset.rs:62-67 · rev-parse процессом vs gix · resolve_commit · польза: меньше процессов, нет ложных ошибок в журнале
- GE-5-01 · упрощение · worktrees.rs:195-231,380-421 · worktrees() считает status каждого worktree ради dirty там, где нужен только branch/path · лёгкий листинг · A/B
- GE-5-02 · упрощение · reflog.rs:112-136 · reflog()/ReflogEntry только для lost_commits · итерировать reflog_of · польза: меньше аллокаций
- GE-6-01 · модули · hooks.rs (549) · hook_run.rs, bypass.rs, commit_template → commit_write.rs · польза: запуск shell в одном месте
- GE-6-02 · модули · worktree.rs:230-296 · add_to_gitignore/delete_untracked → file_ops.rs · польза: модуль статуса только читает

#### app_state и малые крейты (AS)

- AS-1-01 · гонки · crates/app_state/src/lib.rs:592 commit_details · чистое чтение берёт `_quiet` и `forget_row`: каждый клик/↑↓ по коммиту глушит вотчер на ~400 мс и сбрасывает RowCache всех репозиториев · убрать _quiet · низкий · проверено (тест: после commit_details `!watcher_is_quiet`)
- AS-2-01 · баг · crates/diff_engine/src/patch.rs:42-59 (app_state/diffing.rs:109,125) · build_patch всегда «прямой»: Unstage/Discard lines отказывают, если в ханке есть невыбранное изменение · направление в PatchRequest; обратный: невыбранный Insert→контекст, Delete→выбросить · высокий · проверено git 2.51
- AS-2-02 · баг · diff_engine/text.rs:307, patch.rs:69-78 · при пустой стороне ханка start=0 → при context 0 Stage вставки кладёт строку в начало индекса · start = строка перед диапазоном · высокий · проверено
- AS-2-03 · баг · patch.rs:92-94, App.svelte:1279, lib/diff-rows.ts:209 · маркер «No newline» одним флагом в конце патча → в файле без финального LF частичный Stage/Discard отказывает · маркер после каждой строки без LF · средний · проверено
- AS-2-04 · баг · patch.rs:85,118 · одно окончание строк на весь патч; Mixed EOL → частичный Stage отказывает · окончание каждой строки · средний · проверено
- AS-2-05 · баг · patch.rs:106-112 · частичный выбор удалённого/нового файла даёт /dev/null → «still has contents» · /dev/null только при полном выборе · средний · проверено
- AS-2-06 · баг · diff_engine/text.rs:42-55 · lossy-декодирование сводит разные невалидные байты в U+FFFD → «Unchanged» при изменённом файле · не отдавать Unchanged при различии байтов; PUA-отображение · средний · проверено
- AS-2-07 · баг · diff_engine/images.rs:15,24 · «<svg» где угодно в 1 КБ → svg; «BM» в начале → bmp; текстовые файлы (включая 5 компонентов репо) показываются картинкой · строже сигнатуры · средний · проверено
- AS-2-08 · несостыковка · text.rs:180 · Whitespace::All работает как -b, не -w · выбрасывать все пробелы · низкий · проверено
- AS-2-09 · баг INV-12 · app_state/rewrite.rs:167,185,222; network.rs:37 · Moved пишется только при успехе → merge/rebase/cherry-pick с конфликтом без Undo; pull не пишется · head до операции, запись и на Err · средний · проверено
- AS-2-10 · баг · app_state/worktrees.rs:71-79 + safety.rs:104 · Undo «Remove worktree» применяет stash в дереве владельца · восстанавливать worktree или неотменяемо · средний · проверено
- AS-2-11 · баг · app_state/logging.rs:7-15 · COGIT_CRATES без avatars → warn! avatars не в логе · добавить · низкий · проверено
- AS-2-12 · баг · app_state/presets.rs:98-110 · quote = Debug, не TOML; ''' в скрипте → """ портит · toml-крейт · низкий · проверено
- AS-2-13 · баг · app_state/settings.rs:45-48 · BOM → damaged → настройки сброшены · срезать BOM · низкий · проверено
- AS-2-14 · баг фикстуры · test_fixtures/lib.rs:275,290 · with_remote кладёт origin.git в рабочее дерево → тест dirty вхолостую · в _aux · низкий · проверено
- AS-2-15 · несостыковка · test_fixtures/lib.rs:160-180, crates/CLAUDE.md · код под тестом читает глобальный конфиг разработчика · GIT_CONFIG_GLOBAL/NOSYSTEM на прогон · средний · проверено
- AS-2-16 · несостыковка · avatars/queue.rs:96-98 · noreply: запись index.json на каждый request · lookup сначала · низкий · проверено
- AS-2-17 · баг · app_state/terminal.rs:83-90 vs desktop.rs:204 · Git Bash только из Program Files · find_git_bash · низкий · проверено
- AS-2-18 · баг · diff_engine/language.rs:21 + syntax.rs:11 · .tsx парсится TS-грамматикой → синтаксическое слияние не работает · по расширению · низкий · проверено
- AS-2-19 · баг · diff_engine/moves.rs:46-58 · внутрифайловые перемещения без taken · общий taken · низкий · проверено
- AS-2-20 · баг · fs_watcher/watcher.rs:64-79 + lib.rs:16 · hooks/ и rebase-merge/ вне корня (сабмодуль, worktree) не видны · наблюдать hooks · низкий · проверено
- AS-2-21 · зависание · diff_engine/words.rs:10 · word-diff без предела длины → минуты на минифицированных строках · предел/deadline · средний · вероятно
- AS-2-22 · док · doc/01 §2,4.1,4.3; doc/08 §2,5,11 · устаревшие имена и пороги · привести к коду · низкий · проверено
- AS-2-23 · баг (найден при правке AS-2-06) · app_state/diffing.rs selection_patch · в файле не UTF-8 выделенная строка уходила в патч с символом-заменой вместо исходного байта: Stage/Discard lines молча меняли содержимое файла · построчно такой файл не накладывать, только целиком · высокий · проверено
- AS-3-01 · неиспользуемое · diff_engine/lib.rs:49 DiffOptions.ignore_blank_lines · движок не читает · удалить · польза: нет ложной опции
- AS-3-02 · неиспользуемое · app_state/lib.rs:409 AppState::tracked · только тесты; id пересекаются с очередью · удалить с тестами · (тесты не удаляю — см. решение)
- AS-3-03 · неиспользуемое · app_state/lib.rs:1240 register/register_as · только юнит-тесты, обход find_or_register · тестам — find_or_register · польза: один путь регистрации
- AS-3-04 · неиспользуемое · app_state/lib.rs:92 AppEvent::RepoOpened/RepoClosed · форвардер выбрасывает · удалить (тесты events/open читают) · польза
- AS-3-05 · неиспользуемое · fs_watcher/lib.rs:51 RepoChanged.path + derive · никто не читает · отдавать ChangeKind · польза: без строки на событие
- AS-4-01 · дубль · moves.rs:39-63 / 101-131 · два цикла поиска перемещений разошлись (AS-2-19) · один · польза
- AS-4-02 · дубль · terminal.rs:83 / desktop.rs:204 · два поиска Git Bash (AS-2-17) · один · польза
- AS-5-01 · упрощение, горячий · moves.rs:46 · O(D·C·L) на повторяющихся строках · A/B
- AS-5-02 · упрощение · network.rs:65 → lib.rs:733 delete_merged_branches · branches() на каждую ветку · A/B

#### src-tauri (ST)

- ST-1-01 · гонки · src-tauri/src/commands/presets.rs:47 `install_preset` · пишет файл хука мимо очереди (C1-08 пропустил; нет в WRITERS commands/tests.rs:146) · `mutating(.., Other, "install_preset")` + в WRITERS · низкий · проверено
- ST-1-02 · гонки · src-tauri/src/commands/hooks.rs:97 `run_hook`, :9 `run_check` · dry-run исполняет хук (lint-staged делает git stash/add) мимо очереди → параллельный commit `index.lock exists` · через `mutating` · низкий · вероятно
- ST-1-03 · гонки · src-tauri/src/lib.rs:471–477 + app_state/src/queue.rs:209 · форвардер снимает блокировку выключения, только если `session_end_blocker()` пуст, а Done уходит раньше release → «1 operation is still running» висит после push · release до emit (C1-17) · низкий · вероятно
- ST-1-04 · гонки · src-tauri/src/commands/network.rs:106,115,128 `has_token/store_token/forget_token` · keyring на воркере tokio без blocking (класс C1-10) · `blocking` · низкий · вероятно
- ST-2-01 · баг · src-tauri/src/menu/mod.rs:503 `rebuild` → `app.set_menu` · Tauri ставит меню приложения всем окнам без своего меню; у compare/merge после `remove_menu()` оно None → после смены шортката в дочерних окнах полная строка меню главного · ставить меню главному окну (`main_window.set_menu`) на Windows/Linux · средний · проверено по исходникам tauri
- ST-2-02 · баг · src-tauri/src/lib.rs:377, webview2.rs:54 · перехват AcceleratorKeyPressed только на main; в Investigate/Blame шорткаты — акселераторы меню, которые при фокусе в странице не срабатывают; `accelerators::virtual_key` не знает стрелок · install_accelerators для дочерних окон + стрелки/Enter/… в virtual_key · средний · проверено по коду
- ST-2-03 · баг · src-tauri/src/menu/mod.rs:113 `CmdOrCtrl+Return` · muda знает только ENTER, ошибка глотается → Ctrl+Enter на Commit… не привязан · `CmdOrCtrl+Enter`; фронтовый NAMED.Enter · низкий · проверено
- ST-2-04 · несостыковка · commands/mod.rs:119 `set_keymap` + frontend/src/lib/keymap.ts:52 · редактор пишет event.key (русская раскладка → `CmdOrCtrl+Ы`), бэк принимает без проверки, клавиша молча не работает · проверять в set_keymap через accelerators::parse; редактор по event.code · низкий · проверено
- ST-2-05 · баг · src-tauri/src/webview2.rs:105 · AltGr = LCtrl+RAlt → AltGr+S (ś) совпадает с CmdOrCtrl+Alt+S, символ съедается, запускается Stash Selection · не заявлять чорд при VK_RMENU · низкий · проверено по коду
- ST-2-06 · баг · src-tauri/src/renderer_failure.rs:51,77 · GPU/utility-процессы → «other-process-exited», can_reload=true → перезагрузка страницы (теряется набранное сообщение) при сбросе GPU · перезагружать только при RENDER_PROCESS_EXITED · низкий · проверено
- ST-2-07 · баг · src-tauri/src/lib.rs:321 · закрытие главного окна при открытом дочернем не завершает процесс → поиски не отменяются, git не останавливается, второй запуск — второй процесс · на Destroyed у main — app.exit(0) · низкий · проверено по семантике tauri
- ST-2-08 · баг · src-tauri/src/lib.rs:333/349 `log_path`, diagnostics.rs:33, renderer_failure.rs:273 · путь лога запомнен на часть 1; после ротации Copy Diagnostics отдаёт не тот файл · искать последнюю часть сессии в момент копирования · низкий · проверено
- ST-2-09 · несостыковка · commands/mod.rs:708 `command_log` · синхронная на потоке окна (MAIN_THREAD_ONLY), клонирует весь журнал (до 100×1 МБ) на каждый command-recorded при открытом Output · `(async)` · низкий · проверено
- ST-2-10 · несостыковка · renderer_failure.rs:211–249 · тексты диалогов на русском, CLAUDE.md: UI strings English · перевести · низкий · проверено
- ST-2-11 · баг · src-tauri/build.rs:70 `watch_git_state` · при ветке только в packed-refs commit не перезапускает build.rs, About показывает прошлый коммит · наблюдать logs/HEAD · низкий · проверено
- ST-2-12 · несостыковка · lib.rs:314, 380 · window-state с VISIBLE сам показывает окно до setup; комментарии «before the window is shown» неверны · `StateFlags::all() - VISIBLE` · низкий · проверено по исходникам
- ST-2-13 · баг · src-tauri/tauri.debug.conf.json:5 · оверлей заменяет массив windows целиком: пропадают размеры/тема/фон, visible:true; scripts/oom/crash-test.mjs:51 ищет старое имя лога · повторить поля окна; путь · низкий · проверено
- ST-2-14 · несостыковка · commands/mod.rs:107 · doc-комментарий report_timing висит над default_keymap (и в bindings.ts) · перенести · низкий · проверено
- ST-2-15 · док · doc/04-ipc-contract.md:597 · graph_window описан как null, в коде пустая строка · привести к коду · низкий · проверено
- ST-2-16 · баг · webview2.rs:95 · автоповтор клавиши не отсекается → удержание Ctrl+Shift+O ставит десятки push · пропускать WasKeyDown · низкий · вероятно
- ST-2-17 · несостыковка · webview_memory.rs:82, renderer_failure.rs:178 · семплер только в debug, комментарий обещает цифры в релизе; обход процессов в async-задаче · поправить комментарий; spawn_blocking · низкий · проверено
- ST-3-01 · неиспользуемое · tauri.conf.json:29 · CSP разрешает asset: при выключенном asset-протоколе · убрать · польза: CSP ровно по использованию · проверено
- ST-3-02 · неиспользуемое · frontend/package.json:29 `@tauri-apps/plugin-window-state` · JS-пакет не импортируется · удалить · польза: минус зависимость · проверено
- ST-4-01 · дубль · commands/network.rs:19–89, ref_ops.rs:141 · 4 одинаковых тела PhaseTimer+progress+profile::network; уже разошлись (push_to пишется как "push") · хелпер · польза: одно место · проверено
- ST-4-02 · дубль · tauri.conf.json:4 version · дублирует Cargo.toml · удалить поле · польза: версии не разойдутся · проверено
- ST-6-01 · модули · src-tauri/src/lib.rs (499) · события: 7 DTO + forward_repo_changes → events.rs · польза: контракт событий в одном файле · проверено

#### Сторы и lib фронта (FS)

- FS-1-01 · гонки · stores/worktrees.svelte.ts:93-99 (worktree:60-69, stashes:24-47, conflicts:54-65, flow:27-39) · мутации после await сами зовут refresh/load для прежнего репозитория после clear() → данные A в B; worktrees ставит repo=A; Drop stash удаляет чужой · счётчик сбросов до await · средний · проверено
- FS-1-02 · гонки · stores/graph.svelte.ts:112-115,206 · entry(index) при уже запрошенном блоке отдаёт undefined · Map<index,Promise> · низкий · проверено
- FS-1-03 · гонки · stores/diff.svelte.ts:76,89-92,116 · path/spec новой загрузки ставятся сразу, diff остаётся старым → discardLines шлёт path b с хунками a · показанный отдельно от запрошенного · низкий · проверено
- FS-1-04 · гонки · stores/submodules.svelte.ts:79-88 · refresh заменяет children снимком до await → раскрытый за это время узел пропадает; побеждает последний завершившийся · слияние + ticket · низкий · проверено
- FS-1-05 · гонки · stores/network.svelte.ts:64-87 (= FA-1-08) · общий running/progress; колбэк пишет после clear() · по операциям · низкий · проверено
- FS-1-06 · гонки · stores/conflicts.svelte.ts:54-65 · take/write закрывают файл, открытый за это время · закрывать, если path тот же · низкий · проверено
- FS-1-07 · гонки · lib/investigate/session.svelte.ts:196-202 · #reloadSections без поколения · поколение · низкий · проверено
- FS-2-01 · баг · stores/notices.svelte.ts:40,102 (App.svelte:407-412) · report() в $effect читает и пишет #errors → (а) не-command ошибка: ~1000 прогонов и effect_update_depth_exceeded; (б) закрытое уведомление возвращается · untrack · высокий · проверено моделью svelte 5.57.1
- FS-2-02 · баг · CompareWindow.svelte:15-18, BlameWindow.svelte:31-34 (diff.svelte.ts:74-87, settings.svelte.ts:32-41) (= FA-2-18) · settings.load() каждый раз новый объект → бесконечный цикл diff_file; blame дважды · onMount/untrack; settings.load идемпотентный · высокий · проверено моделью
- FS-2-03 · проглочено · stores/hooks.svelte.ts:28,152 · hooks.error не сбрасывается; заголовок «Could not read» для записи · сбрасывать · низкий · проверено
- FS-2-04 · проглочено · stores/compare-view.svelte.ts:30, stash-view.svelte.ts:27 · error не показывается; «Both commits have the same files.» при ошибке · report + текст · низкий · проверено
- FS-2-05 · баг · stores/graph.svelte.ts:164 · clear() не обнуляет skipped · обнулить · низкий · проверено
- FS-2-06 · несостыковка · lib/format.ts:62, lib/ref-nodes.ts:73,201 · remote-ветка режется по первому «/» (remote team/fork) · по списку remotes (splitUpstream) · низкий · проверено
- FS-2-07 · баг · lib/child-window.ts:14 (App:925, FileList:93) · Ctrl+W по event.key → в русской раскладке не закрывает · event.code · низкий · проверено
- FS-2-08 · несостыковка · lib/updates.ts:16 (= FA-2-23) · синхронный интерфейс → window.confirm/alert · Promise · низкий · проверено
- FS-2-09 · баг · lib/pull-request.ts:16 · ssh://git@github.com/o/r.git не разбирается · третий вариант · низкий · проверено
- FS-2-10 · несостыковка · lib/query.ts:68-70 · since:/until: от полуночи UTC, git — от локальной · на решение · низкий · проверено
- FS-3-01 · неиспользуемое · frontend/package.json:29 (= ST-3-02) · @tauri-apps/plugin-window-state · удалить · польза
- FS-3-02 · неиспользуемое · settings.reset/preview, safety.forRepo, files-view.reset, submodules.entries, refs.filter, layout.hidden, stash-view.index, ipc isCommandFailure · удалить · польза: нет ложного второго пути
- FS-4-01 · дубли · 10 сторов + blame-window + investigate/session vs lib/notices.ts:68 · toCogitError ×10, разошлись · одна функция · польза: одна формулировка
- FS-4-02 · дубли · lib/availability.ts vs lib/toolbar.ts:339-413 · две системы доступности, разошлись (Stash в палитре при чистом дереве) · одни правила · польза
- FS-4-03 · дубли · lib/ref-menus.ts:312 localNameOf vs push-to.ts:72 splitUpstream · одна · польза
- FS-4-04 · дубли · stores/confirm, prompt, stash-dialog · четыре копии «ожидаемой модалки» · asked<T>() · польза
- FS-5-01 · упрощение · stores/worktree.svelte.ts:60-70 · mutate глотает ошибку в error, отчёт держится на порядке микрозадач · бросать · польза
- FS-6-01 · модули · lib/ipc/index.ts (923) · группы по модулям commands · польза
- FS-6-02 · модули · lib/toolbar.ts (451) · реестр / меню Pull-Sync / доступность · польза

#### App.svelte, окна, layout (FA)

- FA-1-01 · гонки · App.svelte:1481 runRemoteSteps, 1474 pushOnce, 1505 fetchRemotes · нет эпохи; pushOnce читает network.primary нового репозитория → Sync в A, клик по B → A пушится в remote B · remote/prefs до первого шага, эпоха после каждого await · средний · проверено
- FA-1-02 · гонки · App.svelte:1209,1237,1250,1391,1410,1428,1440,1523,1534,1573,1584,1603,1799,1889,2197 · afterRefChange снимает эпоху после долгой операции → чистит commit/diff нового репозитория · эпоху снимать в начале вызывающего, передавать · низкий · проверено
- FA-1-03 · гонки · App.svelte:429 mutate + discard 1033, discardFromToolbar 1048, deleteFromDisk 1061, askMove 2101, removeFiles 2123, saveIndexEditor 2090 · после подтверждения mutate читает repository.current заново; drop папки под модальным окном меняет репозиторий → discard README.md в B · id снимать до вопроса, проверять эпоху · высокий · проверено
- FA-1-04 · гонки · App.svelte:1926 saveConfig, 3464–3507 HooksPanel, 3603 RepoSettingsDialog, 1876 runRebase · редакторы берут id при сохранении; activate их не закрывает → config A пишется в .git/config B · хранить repo в редакторе, закрывать при смене эпохи · высокий · проверено
- FA-1-05 · гонки · App.svelte:1307 openModule, 2394 openWorktreeRow · adopt после await без проверки перебивает более новый activate(B) · ticket до await · средний · проверено
- FA-1-06 · гонки · App.svelte:992 + repository.svelte.ts:199 · refresh во время opening выходит → вторая пачка событий теряется, Branches остаются старыми · флаг «перечитать после settle» · средний · вероятно
- FA-1-07 · гонки · App.svelte:993,999,1006 · при выходе по left() метки stale не снимаются, activate их не сбрасывает → точка stale навсегда в B · сброс в forgetPanelsKeepingTheTree · низкий · проверено
- FA-1-08 · гонки · App.svelte:2611 syncListed + stores/network.svelte.ts:75 · один слот running/progress на всё приложение; pull неактивного обнуляет running текущего · по repo · низкий · проверено
- FA-1-09 · гонки · App.svelte:860 runFind + layout/FindObject.svelte:60 · finderBusy не сбрасывается на пустом запросе и в перебитом поиске → «Searching…» навсегда; эффект перезапускает поиск при смене current · сброс, report по token, untrack · низкий · проверено
- FA-2-01 · баг · App.svelte:1779 runDropAction + lib/drop-target.ts:47 · «Merge X into Y»/«Rebase X onto Y» выполняются над HEAD, а не над Y · предлагать только при isHead · высокий · проверено
- FA-2-02 · баг · App.svelte:541 · у команды palette commit пустой run → Local ▸ Commit… и «Commit Staged» ничего не делают · фокус/отправка через CommitBox · средний · проверено
- FA-2-03 · баг · App.svelte:471,494 + stores/safety.svelte.ts:6 · last=null сравнивается с !== undefined → Undo всегда активна; last общий на все репозитории · != null и по текущему repo (forRepo) · низкий · проверено
- FA-2-04 · баг · App.svelte:1279 stageLines · noTrailingNewline: false зашито → Stage lines в файле без конечного LF дописывает LF · считать как discardLines · средний · проверено
- FA-2-05 · баг · App.svelte:444 afterMutation без worktree.load: Take side 3358, resolveText 3366, mergeResolved 2877, stageModeOnly 2782, undoEntry 2750, refreshSubmodule 1295 → файл остаётся unmerged в списке; Undo из журнала не видно · worktree.load в afterMutation · средний · проверено
- FA-2-06 · баг · layout/SettingsPanel.svelte:114 · restarts сравнивает draft с живым value → заметка «after a restart» не появляется · снимок при открытии · низкий · проверено
- FA-2-07 · баг · SettingsPanel.svelte:417 + KeymapEditor.svelte:27 + common/Dialog.svelte:60 · правки раскладки теряются на OK/Esc; Esc при захвате закрывает диалог; Cancel не откатывает · применять в onchange; Dialog пропускает defaultPrevented · средний · проверено
- FA-2-08 · баг · App.svelte:3617 + PromptDialog.svelte:34 · без validate подставляется branchNameProblem → группа с пробелом, имя пресета отклоняются; пустой тег при finish нельзя · явный validate · средний · проверено
- FA-2-09 · баг · App.svelte:2287 · Open Repository here… → Cancel переносит текущий репозиторий в группу · pickRepository → root|null · низкий · проверено
- FA-2-10 · баг · App.svelte:2897 retryOf, 3685 · Retry не смотрит entry.repo и remote → пушится текущий B · по записи · средний · проверено
- FA-2-11 · несостыковка · App.svelte:1454 runNetwork("pull"), 2611 syncListed · ffOnly зашит, remote = primary, тулбар берёт pullMode и upstream · один путь pull · средний · проверено
- FA-2-12 · несостыковка · App.svelte:2799 closeCurrent vs 2572 closeListed · Ctrl+W не переходит к следующему, панели не чистятся · свести к closeListed · низкий · проверено
- FA-2-13 · баг · App.svelte:918 + DiffView.svelte:301 · F6 обрабатывают оба (= FC-2-03) · DiffView по focused · низкий · проверено
- FA-2-14 · баг · App.svelte:909, OutputPanel:23, CommandOutput:110, Dialog:60, HooksPanel:55, SafetyJournal:15, DropMenu:14 · Esc закрывает все слои сразу · верхний слой / defaultPrevented · низкий · проверено
- FA-2-15 · баг · layout/CommandOutput.svelte:109 · немодальное окно перехватывает Ctrl+A/F/C/=/− во всём окне · только при фокусе внутри · низкий · проверено
- FA-2-16 · баг · MergeWindow.svelte:37,47 · ошибка сохранения размонтирует MergeView → ручное разрешение пропадает; String(err) без stderr · держать смонтированным · средний · проверено
- FA-2-17 · несостыковка · MergeWindow.svelte:12, MergeView.svelte:53 · закрытие без вопроса при несохранённом; клавиши §9 (= FC-2-09) · dirty · средний · проверено
- FA-2-18 · баг · CompareWindow.svelte:15 · эффект читает settings.current внутри diff.load → settings.load() присваивает → бесконечный цикл diff_file · untrack/onMount · средний · проверено чтением
- FA-2-19 · баг · App.svelte:3698 · StatusBar без encoding/lineEnding → всегда «UTF-8 • LF» · передавать из diff · низкий · проверено
- FA-2-20 · док · doc/05 vs App.svelte:3138,3704, app.css · Branches/References, сводка статус-бара, высота тулбара · привести doc · низкий · проверено
- FA-2-21 · несостыковка · doc/11 §4–8 vs menu/mod.rs · ~30 обещанных шорткатов не назначены; подсказки тулбара lib/toolbar.ts:73 обещают Ctrl+T/Ctrl+Z/… · назначить или убрать · средний · проверено
- FA-2-22 · баг · commands/investigate.rs:179 + lib.rs:378 (= ST-2-02) · акселераторы дочерних окон при фокусе в странице · install_accelerators · средний · вероятно
- FA-2-23 · несостыковка · App.svelte:276,277,1383,1595 · window.confirm/alert/prompt вопреки frontend/CLAUDE.md · сторы prompt/confirmation · низкий · проверено
- FA-2-24 · баг · App.svelte:1229 offerAutostash · checkout после stash упал → изменения в stash, сообщение не говорит где · pop или сказать · низкий · вероятно
- FA-3-01 · неиспользуемое · layout/CommandOutput.svelte:35,189,339 · technical=true и мёртвый CSS · удалить · польза −20 строк · проверено
- FA-3-02 · неиспользуемое · layout/Panel.svelte:20 · проп empty · удалить · проверено
- FA-3-03 · неиспользуемое · stores/safety.svelte.ts:27 forRepo · 0 вызовов, App дублирует фильтр · использовать · польза один фильтр · проверено
- FA-3-04 · неиспользуемое · app.css · --h-menubar, --r-lg, --sp-8, --t-dialog, --t-medium не читаются · удалить/применить · проверено
- FA-3-05 · неиспользуемое · App.svelte:300,414,894,1746,2052,2770 · устаревшие/не на месте комментарии · перенести/удалить · проверено
- FA-3-06 · неиспользуемое · layout/StateBanner.svelte:44,48, HooksPanel:212,287,427, StartScreen:107 · мёртвые запасные литералы · удалить · проверено
- FA-4-01 · дубли · App.svelte activate/comeBack/openModule/openWorktreeRow · четыре копии «показать репозиторий» · хелпер · польза: эпоха в одном месте · проверено
- FA-4-02 · дубли · App.svelte:1033 discard / 1048 discardFromToolbar · одно подтверждение, разошлось · одна функция · проверено
- FA-4-03 · дубли · App.svelte:1262 stageLines / diff.svelte.ts:116 discardLines · разошлись (FA-2-04) · diff.stageLines · проверено
- FA-4-04 · дубли · App.svelte runNetwork/pullOnce/syncListed/primaryRemote · четыре пути pull · один · проверено
- FA-4-05 · дубли · App.svelte openDropped/openScanned + repository.restore · три цикла «открыть несколько» · один · проверено
- FA-4-06 · дубли · App.svelte:348,351 COMMIT_MIN_PX/WORKTREES_MIN_PX vs app.css, уже не сходятся · getComputedStyle · проверено
- FA-5-01 · упрощение · App.svelte:2965 pushMenuState · IPC на каждое изменение выделения · горячий путь, A/B · проверено
- FA-5-02 · упрощение · App.svelte:3275 onviewchange · два worktree_files на переключение · A/B · проверено
- FA-5-03 · упрощение · SettingsPanel.svelte:183–420 · 20 веток if по key · данные · проверено
- FA-6-01 · модули · App.svelte (3889) · сеть, события диска, файловые команды, repo-tree, rebase, exit → сторы/lib · польза: логика под тестами · проверено

#### Компоненты фронта (FC)

- FC-1-01 · гонки · frontend/src/components/file-list/CommitBox.svelte:43 (+ App.svelte:257, :2785) · при смене репозитория draftKey уже B, template ещё от A → шаблон A кладётся в поле и сохраняется черновиком B · шаблон связать с корнем, игнорировать при корне ≠ draftKey · низкий · проверено
- FC-1-02 · гонки · frontend/src/components/diff/DiffView.svelte:243, :332 · pendingDiscard/selected/revealed переживают смену файла или spec → Discard выбрасывает строки B с номерами A; выделение Unstaged применяется к Staged · сбрасывать при смене diff/spec · средний · проверено
- FC-2-01 · баг · CommitBox.svelte:26 · submit очищает сообщение/Amend/No verify до результата; Cancel в вопросе amend published или падение хука теряет текст; повтор без Amend → новый коммит. То же RefActions.svelte:522/535 · oncommit → Promise<boolean>, очищать при успехе · низкий · проверено
- FC-2-02 · баг · DiffView.svelte:361-377, 427-443 (stores/diff.svelte.ts:60) · Stage/Unstage/Discard одинаковы для workTreeVsIndex и indexVsHead; Discard на Staged обращает патч HEAD→index в рабочем дереве → «выброшенное» уходит в коммит · для рабочего дерева Stage/Discard, для индекса Unstage, остальное disabled · средний · проверено
- FC-2-03 · несостыковка · DiffView.svelte:297-317, 358 · клавиши на window без учёта фокуса панели/модалки/defaultPrevented (F6 двоится, Ctrl+F крадёт фокус у фильтра Files) · проп activePanel + defaultPrevented · низкий · проверено
- FC-2-04 · баг · common/VirtualList.svelte:48-55 · эффект reveal перезапускается на каждой прокрутке (читает scrollTop) → в Blame прокрутка отскакивает к курсору · untrack · низкий · проверено
- FC-2-05 · баг · file-list/FilesToolbar.svelte:283 overflow:hidden + :438 · меню Customize View обрезано полосой, невидимый backdrop съедает клик · position:fixed или убрать overflow · низкий · проверено по CSS
- FC-2-06 · баг · file-list/FileList.svelte:99 + lib/commit-scope.ts:12 · «Commit What You See» считает видимое через matchesMask, а список фильтрует compile(); only=[] = коммит всего индекса → фильтр «modified»: «Commit 0 shown» коммитит всё · отдавать видимые пути Staged; при 0 — выключить · средний · проверено
- FC-2-07 · баг · graph/CommitList.svelte:292 (stores/commit.svelte.ts:27,63) · клик по Working Tree не уводит Files из stash · сброс stashView · низкий · проверено
- FC-2-08 · несостыковка · graph/CommitList.svelte:251 · фильтр без совпадений → «No commits yet», пропадает Working Tree (05 §7, §3.4) · различать пустой запрос, Clear filter · низкий · проверено
- FC-2-09 · несостыковка · diff/MergeView.svelte:55 (MergeWindow.svelte) · нет F6/Ctrl+1..3 (11 §9); Esc/Ctrl+W закрывают без вопроса при несохранённом; Ctrl+S игнорирует ручную правку при left>0 · dirty + вопрос · низкий · проверено
- FC-2-10 · несостыковка · DiffView.svelte:707 · у .bar button нет :disabled (06 §6) · правило · низкий · проверено
- FC-2-11 · баг · panels/CommitDetailsPane.svelte:51, :139, graph/PauseCheckBar.svelte:28 · кнопки без стилей (потеряны при выносе из App в 0d49f07) · вернуть стили · низкий · проверено
- FC-2-12 · несостыковка · RefTree:172, BlameView:65, BlamePanel:314, NavigationPanel:208, WorktreeList:104 · выбор только фоном, без полосы (06 §6) · inset 2px · низкий · проверено
- FC-2-13 · несостыковка · common/Caret:24, Disclosure:53, investigate/DeeperBar:74 · литеральные длительности вместо токенов; pulse без reduced-motion (06 §8) · токены + media · низкий · проверено
- FC-2-14 · баг · common/ConfigEditor.svelte:22-27 · textarea нормализует CRLF→LF: ввести и стереть символ → Save переписывает весь файл в LF · editorSide/forDisk из lib/file-dialogs · низкий · проверено
- FC-2-15 · баг · repo-tree/ScanDialog.svelte:62 · Select All выбирает скрытые фильтром · по shown · низкий · проверено
- FC-2-16 · несостыковка · FilesToolbar.svelte:98, 237 · в узкой панели два пункта переключают renameSources · убрать дубль · низкий · проверено
- FC-3-01 · неиспользуемое · DiffView, FilePane, NavigationPanel, MergeView, CommitBox, GraphFilter, RepositoryList, EmptyState · мёртвые запасные значения var(--x, …) (все токены определены) · удалить · польза: нет литеральных цветов в компонентах · проверено
- FC-3-02 · неиспользуемое · VirtualList `class`, SkeletonRows `height`, Select `id`, Checkbox `children` · неиспользуемые пропсы · удалить · польза: API = использование · проверено
- FC-4-01 · дубли · DiffView.svelte:557-665 · пять копий вывода кусков строки · snippet · горячий рендер — A/B · проверено
- FC-4-02 · дубли · малая кнопка в ≥8 компонентах, disabled расходится 0.4…0.55 · глобальные .btn/.btn-sm · польза: одно состояние disabled · проверено
- FC-5-01 · упрощение · panels/RepositoriesPanel.svelte · прокладка с 12 пропсами · убрать · польза: событие правится в 2 файлах, не в 3 · проверено
- FC-5-02 · упрощение · graph/CommitList.svelte:351,370 · фиктивный `0 as unknown as RepoId` · ранний выход · польза: нет IPC с несуществующим id · проверено
- FC-6-01 · модули · menus/RefActions.svelte (811) · логика → lib/ref-actions.ts · польза: ветки становятся тестируемыми · проверено
- FC-6-02 · модули · diff/DiffView.svelte (1023) · DiffToolbar, DiscardStrip · польза: независимые правки · проверено

## Фаза 2 — исправление

Разбор аудита (решения одной строкой):
- Правило фронта «тесты — для сторов, lib, виртуализации и геометрии, не для разметки»: баг,
  который живёт в App.svelte или разметке, чиню так, чтобы решение ушло в стор/lib и было
  покрыто тестом; если вынести нельзя — не чиню и пишу сюда (как C1F-15/18 в первой итерации).
- Дубли находок разных агентов объединены: FA-1-08 = FS-1-05, FA-2-13 = FC-2-03,
  FA-2-17 = FC-2-09, FA-2-18 = FS-2-02, FA-2-22 = ST-2-02, FA-2-23 = FS-2-08, ST-3-02 = FS-3-01.
- ST-1-03 — это C1-17, решение пользователя (п. 9 первой итерации); довод агента добавлю туда.
- Новое поведение, а не починка (на решение): FA-2-21 (≈30 обещанных, но не назначенных
  шорткатов), GE-2-22 (таймаут и отмена сети), FS-2-10 (полночь UTC в `since:`), ST-2-10
  (перевод диалогов — текст решает пользователь), FA-2-20 (References или Branches).

### Категория 1 — гонки

- [x] 1.1 ST-1-01, ST-1-02: install_preset, run_hook, run_check — через очередь
  > Итог: install_preset, run_hook, run_check идут через `mutating`; все три добавлены в
  > WRITERS теста `commands_that_write_the_repository_wait_for_its_lane` (красный до
  > правки).
- [x] 1.2 ST-1-04: keyring в blocking
  > Итог: has_token/store_token/forget_token — через `blocking`; тест
  > `the_keyring_is_reached_off_the_async_workers` (красный до правки).
- [x] 1.3 AS-1-01: commit_details без тихого окна и без сброса RowCache
  > Итог: `_quiet` у `commit_details` появился в 0d6e587 вместе с тихими окнами мутаций —
  > чтение через gix ничего не пишет. Убран; тест `reading_a_commit_is_not_a_write` в
  > tree_rows.rs (красный до правки).
- [x] 1.4 GE-1-01: rename_stash не теряет записи при сбое посреди
  > Итог: шаги вынесены в `rename_with` с подменяемым запуском git (иначе сбой посреди не
  > воспроизвести); при сбое drop возвращаются уже снятые записи, при сбое store остальные
  > всё равно возвращаются, каждая невозвращённая — в лог с oid. Три юнит-теста, два красных
  > до правки.
- [x] 1.5 FS-1-01: мутации сторов не перечитывают прежний репозиторий после clear
  > Итог: в stashes, worktrees, worktree, flow, conflicts — счётчик `#cleared`, который
  > увеличивает `clear()`; запись, закончившаяся после него, ничего не перечитывает. Тест
  > `mutation-after-leaving.test.ts`: пять случаев красные до правки, шестой (тот же
  > репозиторий перечитывается) зелёный.
- [x] 1.6 FS-1-02: graph.entry ждёт уже идущий запрос блока
  > Итог: `asking` стал `Map<блок, Promise>`: второй запрос того же блока ждёт первый. Тест
  > «loads the row a key press moves to while a scroll is already fetching it» (красный до
  > правки).
- [x] 1.7 FS-1-03, FC-1-02: дифф — показанный файл отдельно от запрошенного; выделение и
  подтверждение Discard сбрасываются со сменой диффа
  > Итог: в сторе `#shown` — файл, которому принадлежат строки на экране; `diff` и картинки
  > присваиваются вместе с ним после обоих await. `discardLines(selected, from)` берёт путь
  > показанного диффа и ничего не делает, если `from` уже заменён. DiffView передаёт дифф, в
  > котором выбраны строки, и сбрасывает выделение, раскрытое и подтверждение Discard при
  > смене файла или стороны индекса; панели подписывают дифф `shownPath`. Два теста в
  > diff.test.ts (красные до правки); сброс в разметке — без теста, защиту даёт проверка в
  > сторе. `stageLines` из App — в шаге 2.23 (FA-2-04/FA-4-03).
- [x] 1.8 FS-1-04: submodules.refresh не затирает раскрытое
  > Итог: `refresh` получил свой ticket (пишет только новейший) и переносит детей узла,
  > раскрытого во время чтения. Два теста в leaving-repository.test.ts (красные до правки).
- [x] 1.9 FS-1-05: сетевые операции по отдельности
  > Итог: стор держит список идущих операций, у каждой своя строка прогресса;
  > `running`/`progress`/`repo` — геттеры по новейшей. Закончившаяся убирает только себя;
  > после `clear()` строки идущей операции никуда не пишутся. `run` отдаёт колбэк прогресса
  > операции (RefActions больше не пишет `network.progress` сам). Тест network.test.ts: три
  > случая, красные до правки.
- [x] 1.10 FS-1-06: conflicts.take/write не закрывают другой файл
  > Итог: take/write закрывают вид, только если открыт тот же файл, что решали. Тест «leaves
  > the next file open» в conflicts.test.ts (красный до правки).
- [x] 1.11 FS-1-07: поколение у #reloadSections
  > Итог: у `#reloadSections` свой счётчик; пишет только новейшее чтение, раздел,
  > добавленный за это время навигацией, сохраняется, после нового start ответ
  > отбрасывается. Тест «keeps the newest re-read» (красный до правки).
- [x] 1.12 FA-1-03, FA-1-04: подтверждения и редакторы привязаны к своему репозиторию
  > Итог: `repository.onLeave` — синхронный слушатель смены эпохи; `lib/leaving.ts` отвечает
  > «отмена» на открытые вопросы (confirm, prompt, stash) и закрывает редактор хуков; App по
  > нему же закрывает свои диалоги о репозитории (config репозитория, index editor, удаление
  > файлов, меню перетаскивания, worktree, настройки репозитория, rebase, split). Тесты:
  > слушатель в repository.test.ts, закрытие в lib/leaving.test.ts (красные до правки);
  > подключение в App — разметка, без теста.
- [x] 1.13 FA-1-01, FA-1-02, FA-1-05: эпоха в сетевых шагах, afterRefChange, openModule
  > Итог: FA-1-01: `remotePlan` в lib/toolbar-prefs решает все remote и шаги Sync/Pull до
  > первого шага, App выполняет готовый план (раньше push брал `network.primary` в момент
  > шага — уже у нового репозитория); три теста (красные до правки). FA-1-02:
  > `afterRefChange(id)` — вызывающие передают репозиторий, в котором сделали изменение, и
  > перезагрузки нет, если на экране другой. FA-1-05: openModule/openWorktreeRow выходят,
  > если эпоха сменилась за время открытия. FA-1-02/05 — проверки в разметке App на уже
  > покрытом тестами механизме эпохи (как C1F-02…04 в первой итерации), своего юнит-теста
  > нет. Хэндл сабмодуля или worktree, открытый и брошенный так, остаётся зарегистрированным
  > до выхода — закрывать его нельзя: тот же путь может быть в списке.
- [x] 1.14 FA-1-06, FA-1-07, FA-1-09: refresh после open, метки stale, finderBusy
  > Итог: три коммита. FA-1-06: `refresh` во время перечитывания того же репозитория ставит
  > флаг и перечитывает ещё раз после него; тест в repository.test.ts (красный до правки).
  > FA-1-09: поиск Find Object вынесен в `stores/finder.svelte.ts`: пустой запрос снимает
  > «Searching…», ошибка перебитого поиска не показывается; эффект FindObject зависит только
  > от запроса (`untrack`). Два теста; поверх старой логики из App оба красные (проверено
  > подменой). FA-1-07: метки stale сбрасываются в слушателе `onLeave` (разметка App,
  > механизм покрыт тестом шага 1.12).
- [x] 1.15 FC-1-01: шаблон коммита A не становится черновиком B
  > Итог: шаблон `commit.template` обнуляется в слушателе `onLeave`, так что пустой черновик
  > B не засевается шаблоном A (разметка App на механизме, покрытом тестом шага 1.12; своего
  > теста нет).

### Категория 2 — баги и несостыковки

- [x] 2.1 AS-2-01…05: патч выделенных строк (направление, пустая сторона, маркер EOF, EOL,
  /dev/null)
  > Итог: `build_patch(request, PatchShape)` переписан: направление (вперёд — по старой
  > стороне, назад — по новой), начала ханков из смещений соседних ханков (при контексте 0
  > сторона без строк называет строку, после которой она стоит), маркер `\ No newline` под
  > каждой строкой без перевода строки (`no_newline` появился и у контекста), `/dev/null` —
  > только если файла на этой стороне нет (app_state узнаёт это через `diff_sides`) и
  > выделение забирает его целиком. Поле `noTrailingNewline` убрано из `PatchRequest`. Семь
  > сквозных тестов с настоящим git в app_state/tests/stage_lines.rs — все красные до
  > правки; тесты diff_engine переведены на новую сигнатуру без ослабления. Одним коммитом:
  > это одна функция, переписанная целиком. Не сделано: AS-2-04 (смешанные EOL) — у строки
  > диффа нет своего окончания, нужна новая форма DiffRow в IPC; `lacksFinalNewline` во
  > фронте остался без вызовов, но с тестами — не удаляю.
- [x] 2.2 GE-2-03, GE-2-04: точные пути и `--pathspec-from-file` в оставшихся командах
  > Итог: в runner — `LITERAL` на уровне модуля и `run_git_literal` /
  > `run_git_reading_literal` / `read_git_literal` / `run_git_bytes_literal`; на них
  > переведены stage_mode (оба ls-files), разрешение конфликта (add), file_log, проверка
  > перед move и дифф apply_commit_file. `commit --only` идёт через `run_git_paths` — точные
  > пути и stdin для длинного списка (GE-2-04). Пять тестов в tests/literal_paths.rs, все
  > красные до правки (длинный список — 2500 путей, os error 206). Сабмодули и subtree не
  > трогал: их нет в находке, а пути там — из .gitmodules, а не выбор пользователя.
- [x] 2.3 GE-2-06, GE-2-07, GE-2-08: interactive rebase — многострочный reword, squash,
  merge-коммиты
  > Итог: одним коммитом (три бага одного модуля, у каждого свой тест, все пять тестов
  > красные до правки). GE-2-06: сообщение из нескольких строк идёт в exec через `printf %b`
  > с экранированными переводами строк, однострочное — как было. GE-2-07: перед записью todo
  > `settled_messages` сравнивает сообщение строки с сообщением коммита: не изменённое не
  > пишется (squash сохраняет сообщение, собранное git), новая тема без тела при reword
  > оставляет тело коммита. GE-2-08: план по умолчанию — `--no-merges --topo-order`, как у
  > самого git; edit_author отказывает до начала, если после коммита есть merge (иначе
  > rebase выпрямил бы его). Замечено попутно: reword пустого коммита падает («would make it
  > empty») — было и раньше, не трогал.
- [x] 2.4 GE-2-01, GE-2-02: .gitignore — байты и экранирование
  > Итог: одна функция, один коммит. GE-2-01: .gitignore читается байтами, нечитаемый (кроме
  > отсутствующего) — ошибка, новые строки дописываются через append. GE-2-02:
  > `ignore_pattern` — ведущий `/` и экранирование `\ [ ] * ?` и хвостовых пробелов. Три
  > теста в tests/worktree.rs, красные до правки; четыре прежних проходят без изменений.
- [x] 2.5 GE-2-05: apply_commit_file не зависит от diff.* пользователя
  > Итог: патч коммита с родителем тоже через plumbing `diff-tree -p`, как уже было для
  > корневого; тест с `diff.noprefix` и `color.ui=always` в tests/file_ops.rs (красный до
  > правки).
- [x] 2.6 GE-2-09: split_off не выпрямляет merge
  > Итог: rebase после разделения идёт с `--rebase-merges`; тест «a merge after the split
  > commit stays a merge» (красный до правки).
- [x] 2.7 GE-2-10: surgery читает через read_git
  > Итог: список файлов коммита (`diff-tree -z`), `ls-tree -z` и `for-each-ref --contains`
  > читаются через `read_git` (полностью, без маскировки секретов журналом); checkout и rm
  > выбранных файлов — с точными путями. Тест с файлами `api/token.rs` и
  > `data/year=2024/a.csv` (красный до правки).
- [x] 2.8 GE-2-11: ожидаемые отказы проверок — мимо журнала
  > Итог: существование ветки (flow) — через gix `find_reference`, «отслеживается ли путь»
  > (move, stage_mode) — по индексу gix (`index_blob`, `tracks`); stage_mode теперь берёт
  > blob стадии 0, а для файла в конфликте отказывает (раньше молча записывал blob базы).
  > Четыре теста в tests/probes.rs, три красные до правки, четвёртый страхует переименование
  > папки. Не сделано: `stash_apply` с запасным вариантом без `--index` — не нашёл сценария,
  > где `--index` падает, а запасной проходит, чтобы написать тест (класс B-25 первой
  > итерации).
- [x] 2.9 GE-2-12, GE-2-13, GE-2-14: lost commits — теги, reflog веток, shallow
  > Итог: GE-2-12: достижимость для Lost Commits считается от вершин графа и всех тегов
  > (`keeping_tips`; сам граф от тегов по-прежнему не ходит); тест «a commit only a tag
  > holds is not lost» (красный до правки). GE-2-14: не воспроизвелось — тест с shallow-
  > клоном и догруженной вершиной проходит на старом коде (gix видит shallow-границу);
  > оставлен как страховка. GE-2-13 (reflog веток кроме HEAD) — на решение: чтение reflog
  > каждой ветки на каждом обновлении панели меняет стоимость обновления, без A/B не делаю.
- [x] 2.10 GE-2-15: worktree bare-репозитория
  > Итог: `main_root`: общий каталог bare-репозитория (имя не `.git`) и есть главный;
  > открывается gix только в этом случае. Тест в tests/worktrees.rs (красный до правки).
- [x] 2.11 GE-2-16, GE-2-17: `~` в путях конфига; окружение dry-run хуков
  > Итог: одним коммитом (оба в hooks.rs). GE-2-16: `core.hooksPath` и `commit.template`
  > читаются через gix `trusted_path` (раскрывает `~/`), при ошибке раскрытия — как
  > написано, с предупреждением в лог. GE-2-17: `clear_inherited_git_vars` из runner
  > применяется и к bash хуков, и к команде проверки; тест через новый тестовый бинарь
  > `probe-hook` (как `probe-status` для R-22) с чужим GIT_DIR в окружении. Оба теста
  > красные до правки; тест шаблона кладёт файл в домашнюю папку и удаляет его. Не сделано:
  > поиск bash рядом с git вместо PATH (WSL-bash) — «вероятно», без воспроизведения.
- [x] 2.12 GE-2-18: повторный finish после конфликта
  > Итог: тег не создаётся, если он уже указывает на вершину главной ветки (`tags_tip` через
  > gix, аннотированный тег разыменовывается до коммита); тег в другом месте — прежняя
  > ошибка git. Тест «a release finished again after a conflict completes» (красный до
  > правки).
- [x] 2.13 GE-2-19, 20, 21, 23, 24, 25, 26: мелкие git_engine
  > Итог: шесть коммитов, у каждого тест, красный до правки: GE-2-19 rename_tag копирует
  > аннотацию с `--cleanup=verbatim`; GE-2-20 выполненные шаги rebase считаются тем же
  > фильтром, что оставшиеся (без exec/break); GE-2-21 процент переименования — `similarity`
  > gix, как `R<nnn>` у git (тест сверяет с `git diff -M`: было 56 против 51); GE-2-24
  > неудачное байтовое чтение пишется в журнал под номером из ошибки (юнит-тест в
  > runner.rs); GE-2-25 номер stash — место записи в reflog, как у git `stash@{n}` (тест с
  > цепочкой, которую git сам принимает; так же нумеруется `HEAD@{n}`); GE-2-26 фаза «Delta
  > compression» при любом числе потоков. Не сделано: GE-2-23 (stdin у сетевых команд) — под
  > тест-раннером stdin уже пуст, воспроизвести нельзя; касается только dev-сборки с
  > консолью.
- [x] 2.14 AS-2-06, AS-2-07, AS-2-08: diff_engine — невалидный UTF-8, ложные картинки, -w
  > Итог: четыре коммита, у каждого тест, красный до правки. AS-2-06: байт не UTF-8
  > декодируется в свой символ Private Use (U+F700 + байт), а не в общий U+FFFD — изменённый
  > файл больше не «Unchanged». AS-2-08: «Ignore all whitespace» убирает все пробелы, как
  > `git diff -w`. AS-2-07: SVG — только если документ начинается с `<svg` (после BOM,
  > пролога, комментариев, doctype), BMP — по заголовку (зарезервированные нули и размер
  > DIB); тесты-страховки на настоящие SVG с прологом и BMP проходят и до, и после. Новая
  > находка AS-2-23 (записана в находки): построчный Stage/Discard в файле не UTF-8 писал
  > символ-замену вместо байта — теперь отказ с объяснением; тест со сквозным git (красный
  > до правки).
- [x] 2.15 AS-2-09, AS-2-10: Undo после конфликта, Undo удаления worktree
  > Итог: два коммита. AS-2-09: `record_move` записывает Moved и когда операция остановилась
  > на конфликте (`is_interrupted_operation` — заодно у него появился вызов, см. GE-3-03);
  > pull тоже пишется в журнал. Тесты: merge с конфликтом, законченный коммитом,
  > откатывается; pull откатывается (оба красные до правки). AS-2-10: новый
  > `Recovery::Worktree` — Undo заново добавляет worktree на его ветку или коммит и
  > применяет stash в нём, а не в дереве владельца; прежний тест «undoable» проходит без
  > изменений, новый — красный до правки.
- [x] 2.16 AS-2-11, 12, 13, 16, 17: мелкие app_state и avatars
  > Итог: пять коммитов, у каждого тест, красный до правки: AS-2-11 avatars в фильтре лога;
  > AS-2-13 BOM в settings.json снимается перед разбором; AS-2-12 пресет пишется библиотекой
  > toml (`git_engine::preset_toml` — в app_state новых зависимостей нет), имя с эмодзи и
  > скрипт с `'''` и `\d` читаются обратно как были; AS-2-16 noreply-адрес, уже известный
  > кэшу, больше не переписывает index.json на каждой прокрутке; AS-2-17 bash для терминала
  > Git Bash — рядом с `git-bash.exe`, найденным `find_git_bash` (`terminal::bash_of`).
- [x] 2.17 AS-2-14, AS-2-15: фикстуры — remote вне дерева, глобальный конфиг
  > Итог: AS-2-14: bare-remote и второй клон `with_remote` — во вспомогательном TempDir
  > рядом с репозиторием; тест «a repository with a remote starts clean» (красный до
  > правки), весь воркспейс зелёный. AS-2-15 — на решение: код под тестом читает глобальный
  > конфиг разработчика; задать `GIT_CONFIG_GLOBAL`/`GIT_CONFIG_NOSYSTEM` на весь прогон
  > можно только setup-скриптом nextest (экспериментальная возможность) — в процессе теста
  > `set_var` запрещён (`unsafe_code = deny`), а `.cargo/config.toml [env]` задел бы и
  > `cargo run` приложения. Строка crates/CLAUDE.md верна: там речь о git-вызовах самих
  > фикстур.
- [x] 2.18 AS-2-18, 19, 20, 21: tsx, перемещения, хуки вне корня, word-diff длинных строк
  > Итог: три исправления и одна страховка. AS-2-18: `merge_grammar_for_path` отдаёт
  > трёхстороннему слиянию грамматику tree-sitter по пути (для `.tsx` — TSX, которая знает
  > JSX); тест с правками вплотную, которые без синтаксиса конфликтуют (красный с
  > грамматикой TypeScript). AS-2-19: вставка, уже ставшая концом одного перемещения, не
  > достаётся второй удалённой копии; тест (красный до правки); отдельным коммитом ограничил
  > проверку занятых вставок длиной совпадения, чтобы не добавить квадратичный проход.
  > AS-2-20: хуки в git-каталоге вне корня (сабмодуль, общий каталог worktree) наблюдаются
  > рекурсивно; тест (красный до правки). AS-2-21: не воспроизвелось — две строки по 40 000
  > разных слов считаются за 0,1 с; тест оставлен страховкой (порог 5 с).
- [x] 2.19 ST-2-*: src-tauri (меню дочерних окон, акселераторы, Return, раскладка, AltGr,
  GPU-сбой, выход, лог, command_log, build.rs, window-state, debug-конфиг, автоповтор)
  > Итог: сделано шесть, у каждого тест, красный до правки: ST-2-06 перезагрузка страницы
  > только при потере рендерера, не GPU/utility-процесса; ST-2-09 `command_log` — `(async)`,
  > убран из MAIN_THREAD_ONLY; ST-2-13 отладочный оверлей повторяет все поля главного окна
  > (тест сверяет оба оверлея с базовым конфигом), в crash-test.mjs — имя лога по сессии;
  > ST-2-05 `chord_of`: Ctrl+правый Alt (AltGr) — ввод символа, не шорткат; ST-2-04 (часть
  > фронта) редактор клавиш пишет букву и цифру по `event.code` (на русской раскладке Ctrl+S
  > больше не «CmdOrCtrl+Ы»); ST-2-08 Copy Diagnostics и «Открыть лог» берут текущую часть
  > лога сессии (`logging::latest_part`). Комментарии: ST-2-14 перенесён на report_timing,
  > ST-2-17 уточнён (цифры памяти — только debug). Не сделано, на решение или без способа
  > проверить: ST-2-01 (меню главного окна в Compare/Merge после смены клавиш — чинится
  > `main_window.set_menu` вместо `app.set_menu`, но без тестового рантайма tauri не
  > проверить); ST-2-02 (акселераторы в окнах Investigate/Blame и стрелки в `virtual_key` —
  > новая подписка на окна); ST-2-03 вместе с FA-2-02 (Ctrl+Enter/«Commit…»: как только окно
  > начнёт перехватывать Ctrl+Enter, пустая команда меню сломает коммит из поля сообщения —
  > сначала решить, что делает «Commit…»; тест фронта закрепляет `Return`); ST-2-04 проверка
  > на бэкенде (что считать допустимым — только то, что умеет `virtual_key`?); ST-2-07
  > (выход при закрытии главного окна — событие цикла приложения, без тестового рантайма);
  > ST-2-11 (build.rs), ST-2-12 (флаги window-state меняют момент показа окна) — без способа
  > проверить тестом; ST-2-16 (автоповтор: какие команды должны повторяться при удержании —
  > решение); ST-2-10 (тексты диалогов) — решение пользователя; ST-2-15 — в шаге 2.25.
- [x] 2.20 FS-2-01, FS-2-02: циклы эффектов (уведомления, окна Compare/Blame)
  > Итог: FS-2-02: `settings.load` оставляет прежние объекты, если файл не менялся (тест в
  > stores/settings.test.ts, красный до правки), а окна Compare и Blame делают начальную
  > загрузку в `untrack` — цикл `diff_file` и двойной blame ушли. FS-2-01: `notices.report`
  > и проверка в `command` читают очередь в `untrack` — без юнит-теста: vitest здесь
  > работает в node с серверной сборкой Svelte, где `$effect` не выполняется; три попытки
  > включить клиентскую сборку без новых зависимостей (своё окружение vitest,
  > `resolve.conditions`, inline svelte) не сработали. Нужен jsdom или happy-dom — это новая
  > dev-зависимость, на решение пользователя.
- [x] 2.21 FS-2-03…09: мелкие сторы и lib
  > Итог: шесть коммитов, у каждого тест, красный до правки (кроме FS-2-04): FS-2-03
  > действие над хуками сбрасывает прошлую ошибку, заголовок уведомления — по действию
  > (`hooks.failure`); FS-2-04 ошибки compareView и stashView уходят в уведомления (два
  > эффекта в App рядом с остальными — на механизме `errors.report`, своего теста нет; текст
  > «Both commits have the same files.» при ошибке в разметке не менял); FS-2-05
  > `graph.clear` обнуляет `skipped`; FS-2-07 Ctrl+W в дочерних окнах по `event.code`;
  > FS-2-08 вопрос об обновлении ждёт ответа (`await io.confirm`) и задаётся диалогом Tauri,
  > как остальные вопросы App, а не `window.confirm/alert` (закрывает и FA-2-23 для этой
  > пары); FS-2-09 разбор `ssh://` с портом и без. Не сделано: FS-2-06 (remote с `/` в
  > имени) — у Branch нет имени remote, нужен новый поле в DTO; FS-2-10 (полночь UTC) —
  > решение пользователя.
- [x] 2.22 FA-2-01: merge/rebase из перетаскивания — над названной веткой
  > Итог: `dropActions(..., head)`: слияние предлагается включённым только в отмеченную
  > ветку, rebase — только отмеченной; иначе пункт остаётся в меню выключенным с причиной в
  > подсказке (правило фронта: неприменимое — disabled, не скрыто). App передаёт текущую
  > ветку и не выполняет выключенный пункт; DropMenu рисует выключенный с непрозрачностью
  > 0,4 (06 §6). Два теста в drop-target.test.ts, красные до правки; прежние семь проходят
  > без изменений.
- [x] 2.23 FA-2-02…19, 24: App и окна
  > Итог: сделано десять. С тестом (красным до правки): FA-2-03 `safety.lastFor(repo)` —
  > Undo только текущего репозитория, кнопка выключена, когда нечего; FA-2-10 `lib/retry.ts`
  > — Retry только в том репозитории и к тому remote, где команда упала (первый коммит ушёл
  > с испорченным тестом из-за съеденного heredoc обратного слэша, следующий коммит его
  > чинит); FA-2-19 `fileFormat` — статус-бар показывает кодировку и окончания строк файла;
  > FA-2-08 правила ввода в `lib/names.ts`, `validate` у prompt стал обязательным, каждый
  > вызов называет своё (группы, пресеты, stash — любой непустой текст, тег при Finish —
  > можно пустым, ветки — правило ветки, тег при переименовании — git проверит сам).
  > Проводка App без своего теста: FA-2-05 `afterWorkingTreeChange` после Take side/ручного
  > разрешения/merge-resolved/Undo из журнала/exec-бита/сабмодуля; FA-2-09 «Open Repository
  > here…» при отмене ничего не переносит (`activate` и `pickRepository` возвращают открытый
  > корень); FA-2-23 два оставшихся `window.prompt` → `prompt.ask`; FA-2-24 неудачный
  > checkout после autostash возвращает изменения из stash; FA-2-04 закрыт шагом 2.1 (маркер
  > EOF по строкам); FA-2-18 = FS-2-02. Не сделано: FA-2-02 (вместе с ST-2-03), FA-2-11 и
  > FA-2-21 — решение; FA-2-06, 07, 12, 13, 14, 15, 16 — разметка без способа написать тест
  > (эффекты не выполняются в тестовой среде, см. 2.20); FA-2-17 = FC-2-09; FA-2-20 — шаг
  > 2.25; FA-2-22 = ST-2-02.
- [x] 2.24 FC-2-01…16: компоненты
  > Итог: сделано девять. С тестом (красным до правки): FC-2-02 `diff.lineActions` по
  > стороне индекса (Stage/Discard — рабочее дерево, Unstage — индекс), остальные кнопки
  > выключены, а `discardLines` отказывает на диффе индекса; FC-2-06 `commitScope` берёт
  > строки, которые список действительно показывает (FileList отдаёт их через `onshown`), и
  > `empty`, когда фильтр скрыл все — коммит тогда не идёт (пустой список путей = весь
  > индекс); FC-2-15 Select All/None в скане — по видимым строкам; FC-2-07
  > `commit.showWorkingTree()` уводит Files и из stash. Без своего теста (компонент/CSS):
  > FC-2-01 CommitBox очищает сообщение, Amend и No verify только после удачного коммита
  > (`commitStaged` возвращает `false`, если коммита не было); FC-2-04 VirtualList берёт
  > позицию из элемента, эффект зависит только от `reveal`; FC-2-10 выключенные кнопки диффа
  > — непрозрачность 0,4, без подсветки; FC-2-11 стили кнопок CommitDetailsPane (потеряны в
  > 0d49f07) и Run в PauseCheckBar; FC-2-13 токены анимации в Caret/Disclosure и reduced-
  > motion для пульса DeeperBar. Не подтвердилось: FC-2-14 — бэкенд отдаёт текст конфига уже
  > с \n и сам возвращает CRLF при записи (`ConfigFile.crlf`). Не сделано: FC-2-03 (=
  > FA-2-13), FC-2-05, 08, 09, 12, 16 — разметка и вёрстка без способа проверить тестом или
  > без визуальной проверки.
- [x] 2.25 Документы: ST-2-15, AS-2-22
  > Итог: ST-2-15: `graph_window` в 04 §5 — пустая строка, base64, и для чужого репозитория
  > тоже. AS-2-22: 01 — `diff_text`/`diff_bytes`/`diff_one`, graph_engine без rayon,
  > `RepoWatcher::start` и семь видов `ChangeKind`, без крейта ignore, фикстуры — свободные
  > функции, событие открытия в окна не пересылается; 08 — TooLarge с 1 МиБ, word-diff при
  > отношении до 1:100 без предела строк, смещения в UTF-16. FA-2-20 (References или
  > Branches, сводка статус-бара, высота тулбара) — на решение: неясно, что верно — документ
  > или интерфейс.

### Категория 3 — неиспользуемое

- [x] 3.1 Rust: GE-3-01…03, AS-3-01, 03, 04, 05, ST-3-01
  > Итог: пять коммитов, польза в каждом сообщении: GE-3-01 `GitError::RepoBusy` (не
  > создавался нигде) убран из кода, биндингов, мёртвой ветки фронта и контракта; GE-3-02
  > `RepoStatus::total`; AS-3-01 `DiffOptions.ignore_blank_lines` (движок не читал, фронт
  > всегда слал false) — из кода, фронта, 04 и 08; AS-3-05 `RepoChanged.path` — событие
  > несёт только вид, строка на каждое событие больше не строится; ST-3-01 CSP без asset-
  > протокола. GE-3-03 отпал: `is_interrupted_operation` теперь вызывается (шаг 2.15). Не
  > делал: AS-3-02 (`tracked`), AS-3-03 (`register`), AS-3-04 (события
  > RepoOpened/RepoClosed) — ими пользуются тесты, а тесты не удаляю.
- [x] 3.2 Фронт: FS-3-01, FS-3-02, FC-3-01, FC-3-02, FA-3-01…06
  > Итог: семь коммитов: FS-3-02 убраны `settings.reset/preview`, `filesView.reset`,
  > `submodules.entries`, `refs.filter`, `stashView.index`, `isCommandFailure`, а журнал App
  > пользуется `safety.forRepo` вместо своего фильтра (FA-3-03); FS-3-01 JS-пакет window-
  > state (lock обновлён `npm install` из корня); FC-3-01/FA-3-06 запасные значения токенов,
  > заданных во всех темах (литеральные цвета и 3px за ними не использовались ни разу);
  > FA-3-01 постоянно включённый переключатель `technical` и мёртвые стили окна вывода;
  > FA-3-02 проп `empty` у Panel; FC-3-02 пропсы общих компонентов без единого вызывающего;
  > FA-3-05 doc-комментарии переставлены на свои функции, устаревший про окно Investigate
  > убран. Не делал: FA-3-04 — неиспользуемые токены входят в шкалу, описанную в 06
  > (отступы, радиусы, высоты), польза удаления мала; `layout.hidden` оказался используемым
  > внутри стора; JS-запасное `|| "#888"` в NavigationPanel оставил.

### Категория 4 — дубли

- [x] 4.1 Rust: GE-4-01…03, AS-4-01, AS-4-02, ST-4-01, ST-4-02
  > Итог: четыре коммита: GE-4-01 ancestry зовёт `resolve_commit` вместо копии; GE-4-02 одна
  > проверка пустого списка путей (`staging::require_paths`) вместо двух одинаковых;
  > остальные проверки имён говорят разное и остались; ST-4-01 `with_progress` — строки
  > прогресса и профиль сети в одном месте для fetch, pull, push и push_to; ST-4-02 версия
  > только в Cargo, поле из tauri.conf.json убрано (Tauri 2 берёт её из Cargo.toml; проверит
  > релизная сборка в фазе 3). AS-4-02 закрыт шагом 2.16 (терминал ищет Git Bash через
  > `find_git_bash`). Не делал: GE-4-03 (rev-parse процессом → gix — меняет стоимость
  > команд, только с A/B) и AS-4-01 (слияние двух циклов поиска перемещений — алгоритм на
  > горячем пути диффа, только с A/B).
- [x] 4.2 Фронт: FS-4-01…04, FA-4-01…06, FC-4-02
  > Итог: два коммита: FS-4-01 `toCogitError` в $lib/ipc вместо десяти копий в сторах
  > (формулировка прежняя; у уведомлений своя `asCogitError`, она принимает и сырой GitError
  > — оставил); FA-4-03 запрос Stage lines собирается в сторе рядом с Discard (`#lines`), из
  > показанного диффа и только на своей стороне индекса — заодно закрыта половина FS-1-03
  > для Stage (два теста, красные до правки). Не делал: FS-4-02 и FS-4-03 — объединение
  > меняет поведение (палитра начнёт выключать Stash, разбор имён remote), на решение;
  > FA-4-02 — общий вопрос поменял бы текст и заголовок подтверждения с тулбара; FA-4-04,
  > FA-4-05 — пути pull и открытия нескольких репозиториев ведут себя по-разному, объединять
  > — решение; FA-4-01, FA-4-06, FS-4-04, FC-4-02 — переделка App и общих стилей, польза
  > против риска мала без визуальной проверки.

### Категория 5 — упрощение

- [ ] 5.1 FS-5-01, FC-5-01, FC-5-02, FA-5-03, GE-5-02 (не горячие); горячие (FA-5-01, FA-5-02,
  GE-5-01, AS-5-01, AS-5-02, FC-4-01) — только с A/B

### Категория 6 — модули

- [ ] 6.1 ST-6-01, GE-6-01, GE-6-02, FS-6-01, FS-6-02, FC-6-01, FC-6-02, FA-6-01 — чистый перенос

## Фаза 3 — итог
