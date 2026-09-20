# Ветка `diff-merge`: первый заход, только Rust

> **Исполнителю:** задачи идут по порядку, каждая заканчивается зелёным прогоном и
> коммитом. Шаги отмечаются `- [ ]`. TDD обязателен ([09-testing.md](../09-testing.md)):
> сначала падающий тест, потом код.

**Цель:** закрыть всю бэкендовую часть сорока шагов ветки, не трогая фронтенд,
пока `master` не влил декомпозицию `App.svelte`.

**Подход:** `diff_engine` расширяется аддитивно — новые поля DTO с `#[serde(default)]`,
чтобы `bindings.ts` и существующие компоненты Svelte продолжали компилироваться без
единой правки. Blame и Investigate живут в `blame.rs` и `find.rs`, их типы
реэкспортируются ниже нового разделителя в `git_engine/src/lib.rs`.

**Стек:** `imara-diff` 0.2, `gix` 0.87, `rayon` 1.12, `tokio::task::spawn_blocking`.

**Исходник задания:** [2026-09-21-split.md §3](2026-09-21-split.md), шаги 1–18 и 30–35.

## Глобальные ограничения

- Территория ветки — список из [§1 split-документа](2026-09-21-split.md). Ничего вне его.
- Шесть общих файлов правятся **строго ниже разделителя**. Седьмым становится
  `crates/git_engine/src/lib.rs` — разделитель в него добавляет задача 8.
- `frontend/**` не трогаем совсем. Значит, любое изменение DTO — **аддитивное**.
- `frontend/src/lib/ipc/bindings.ts` руками не правится. Хук перегенерирует его сам.
- Номера: фичи `F-200` и далее, риски `R-100` и далее.
- Никаких `.unwrap()` / `.expect()` на путях Git ([INV-07](../01-architecture.md)).
- Комментарии — под 5% строк файла, только неочевидное.
- Коммит: Conventional Commits, английский, одна строка, без трейлеров, scope — модуль.
- `rayon` никогда не вызывается прямо из async-задачи — только внутри `spawn_blocking`
  ([INV-01](../01-architecture.md)).

## Что уже сделано до начала ветки

Проверено чтением кода и списком тестов, а не на глаз. Эти пункты закрываются
подтверждением, а не переписыванием.

| Пункт | Где | Доказательство |
|---|---|---|
| 1 · решение по `@codemirror/merge` | [12-risks.md](../12-risks.md) R-10 | решение принято 20.09.2026: CodeMirror не добавляется |
| 9 · CRLF не как полная переделка | `diff_engine/src/eol.rs` | `a_file_that_differs_only_in_line_endings_is_not_a_rewrite`, `line_endings_are_normalized_before_comparing`, `a_file_with_mixed_line_endings_still_diffs_line_by_line`, `mixed_line_endings_are_reported_as_mixed` |
| 10 · word-diff на многобайтовых | `diff_engine/src/words.rs` | `offsets_are_utf16_units_so_javascript_can_slice_directly`, `an_emoji_replaced_by_an_emoji_keeps_its_boundaries` |
| 13 · порог и нормализация отступов | `diff_engine/src/moves.rs` | `MIN_MOVED_LINES = 3`, `a_block_shorter_than_the_threshold_is_not_a_move`, `indentation_alone_does_not_hide_a_move` |
| 18 · stage 1/2/3 через `gix` | `git_engine/src/conflicts.rs` | `stage_blob()` читает индекс без процесса; `all_three_sides_are_readable_while_the_merge_is_unresolved` |
| 11 · маркер `\ No newline` | наполовину | в `build_patch` есть (`a_missing_final_newline_is_marked`); в `DiffRow` для вьюера нет |

---

## Задача 1 · Закрыть R-10 сравнением (пункт 1)

Решение уже принято, но пункт требует **сравнения** в реестре. R-10 лежит выше
разделителя и правке не подлежит, поэтому вывод оформляется как R-100.

**Файлы:** правка `doc/12-risks.md` — ниже разделителя.

- [ ] **Шаг 1.** Дописать `## R-100 · @codemirror/merge против своего вьюера · С`
      с таблицей по шести требованиям: unified/split, виртуализация на 100 000 строк,
      соединительные сплайны, поиск внутри diff, жёлоб стейджинга, четырёхпанельный
      merge. По каждому — даёт ли готовый компонент, и чего это стоит.
- [ ] **Шаг 2.** Вывод: R-10 закрыт, решение подтверждено, ссылка из R-100 на R-10.
- [ ] **Шаг 3.** Коммит: `docs: close R-10 with the comparison against @codemirror/merge`

## Задача 2 · Бюджеты `diff_engine` (пункт 12)

Идёт раньше пункта 7: сначала измеряем, потом ускоряем. Бюджеты из
[08-diff-engine.md §9](../08-diff-engine.md): 5 000 строк < 50 мс, 100 000 строк < 1 с.
Пачка из 500 файлов < 2 с — в задаче 3, вместе с самой пачкой.

**Файлы:** создать `crates/diff_engine/tests/budget.rs`.

Имя файла важно: `.config/nextest.toml` уже отправляет `test(/budget/)` в группу
`timing` с машиной в одиночку, а `pre-commit` их пропускает. Ничего настраивать не надо.

- [ ] **Шаг 1.** Тест `a_five_thousand_line_file_diffs_inside_the_budget`: генерируем
      две версии по 5 000 строк с правкой каждой сотой, замеряем `diff_text`,
      печатаем число и `assert!(elapsed < Duration::from_millis(50))`.
- [ ] **Шаг 2.** Тест `a_hundred_thousand_line_file_diffs_inside_the_budget`:
      то же на 100 000 строк, бюджет 1 с, `word_diff: false`.
- [ ] **Шаг 3.** Прогон: `cargo nextest run -p diff_engine --test budget`.
      Если не укладывается — это находка, а не повод поднять бюджет: разбираться
      и записывать в риски.
- [ ] **Шаг 4.** Коммит: `test(m7): measure the diff budgets from the spec`

## Задача 3 · Пачка файлов через `rayon` (пункт 7)

Сейчас `AppState::diff_file` считает по одному файлу. Нужен пакетный путь.

**Файлы:**
- Правка `crates/diff_engine/src/lib.rs` — экспорт новой функции.
- Создать `crates/diff_engine/src/batch.rs`.
- Правка `crates/app_state/src/lib.rs` — **ниже разделителя**, метод `diff_files`.
- Правка `src-tauri/src/commands/mod.rs` и `src-tauri/src/lib.rs` — **ниже разделителя**.
- Создать `crates/diff_engine/tests/batch.rs`.

**Интерфейс:**
- Даёт: `pub fn diff_many(inputs: Vec<(String, Vec<u8>, Vec<u8>)>, options: &DiffOptions) -> Vec<(String, FileDiff)>`
  — `par_iter` по файлам, порядок результата совпадает с порядком входа.

- [ ] **Шаг 1.** Тест `a_batch_returns_one_diff_per_file_in_input_order`.
- [ ] **Шаг 2.** Тест `a_batch_of_five_hundred_files_diffs_inside_the_budget` в
      `tests/budget.rs`, бюджет 2 с.
- [ ] **Шаг 3.** Прогон — падает, функции нет.
- [ ] **Шаг 4.** Реализация через `rayon::prelude::*` и `into_par_iter().map(...).collect()`.
      `collect()` в `Vec` сохраняет порядок — на это и опирается тест шага 1.
- [ ] **Шаг 5.** Прогон — зелено.
- [ ] **Шаг 6.** `AppState::diff_files` ниже разделителя; команда Tauri оборачивает
      её в `spawn_blocking` тем же хелпером, что и остальные (см. R-40).
- [ ] **Шаг 7.** `cargo run -p cogit --bin export-bindings`, затем
      `doc/04-ipc-contract.md` — новая команда.
- [ ] **Шаг 8.** `doc/features/F-200-batch-diff.md` + строка в `features/README.md`
      ниже разделителя.
- [ ] **Шаг 9.** Коммит: `feat(m7): diff a commit's files in parallel`

## Задача 4 · Отмена расчёта при смене коммита (пункт 8)

**Файлы:** правка `crates/app_state/src/lib.rs` ниже разделителя;
`src-tauri/src/commands/mod.rs` и `src-tauri/src/lib.rs` ниже разделителя.

Отмена живёт в `app_state`, а не в `diff_engine`: движок обязан остаться чистой
функцией без состояния, иначе его нельзя гонять в тестах параллельно.

- [ ] **Шаг 1.** Тест в `crates/app_state/tests/` : запрос с устаревшим поколением
      возвращает `DiffOutcome::Superseded`, а не результат.
- [ ] **Шаг 2.** Прогон — падает.
- [ ] **Шаг 3.** Реализация: счётчик поколений `AtomicU64` в состоянии; выбор коммита
      его увеличивает; пакетный расчёт проверяет своё поколение между файлами и
      бросает работу. Проверка **между** файлами, а не внутри одного: прерывать
      `imara-diff` на полпути нечем.
- [ ] **Шаг 4.** Прогон — зелено.
- [ ] **Шаг 5.** `doc/04-ipc-contract.md`, `doc/features/F-201-cancel-stale-diff.md`.
- [ ] **Шаг 6.** Коммит: `feat(m7): drop a diff whose commit is no longer selected`

## Задача 5 · Маркер `\ No newline at end of file` во вьюере (пункт 11)

`build_patch` уже его ставит. Вьюер о нём не знает, потому что `strip_newline`
теряет факт, а `DiffRow` его не несёт.

**Файлы:** правка `crates/diff_engine/src/text.rs`, `crates/diff_engine/src/lib.rs`;
правка `crates/diff_engine/tests/text.rs`.

**Интерфейс:** в `DiffRow::Delete` и `DiffRow::Insert` добавляется
`#[serde(default)] pub no_newline: bool`. Аддитивно: старый фронтенд не ломается.

- [ ] **Шаг 1.** Тест `a_last_line_without_a_newline_is_flagged_on_the_side_that_lacks_it`:
      старое `"a\nb"`, новое `"a\nc\n"` — у `Delete` флаг стоит, у `Insert` нет.
- [ ] **Шаг 2.** Тест `both_sides_can_lack_the_final_newline`.
- [ ] **Шаг 3.** Тест `a_file_ending_in_a_newline_flags_nothing`.
- [ ] **Шаг 4.** Прогон — падает, поля нет.
- [ ] **Шаг 5.** Реализация: в `diff_text` запомнить, оканчивается ли каждая сторона
      на `\n`, и выставить флаг на последней строке той стороны.
- [ ] **Шаг 6.** Прогон — зелено, плюс `cargo insta review` для снапшота.
- [ ] **Шаг 7.** `export-bindings`, `doc/04-ipc-contract.md`,
      `doc/features/F-202-no-newline-marker.md`.
- [ ] **Шаг 8.** Коммит: `feat(m7): flag a missing final newline in the diff rows`

## Задача 6 · Парный маркер «откуда → куда» (пункт 14)

Сейчас есть только `moved: bool` — по нему нельзя сказать, какой блок куда уехал.

**Файлы:** правка `crates/diff_engine/src/moves.rs`, `src/lib.rs`, `tests/moves.rs`.

**Интерфейс:** аддитивно, `moved: bool` остаётся. Добавляется
`#[serde(default)] pub move_id: Option<u32>` в `Delete` и `Insert`. Оба конца одного
перемещения несут один и тот же `move_id`.

- [ ] **Шаг 1.** Тест `both_ends_of_a_move_share_one_identifier`.
- [ ] **Шаг 2.** Тест `two_separate_moves_get_different_identifiers` — расширить
      существующий `two_separate_moves_are_both_found`.
- [ ] **Шаг 3.** Тест `an_ordinary_edit_has_no_move_identifier`.
- [ ] **Шаг 4.** Прогон — падает.
- [ ] **Шаг 5.** Реализация: в `detect_moves` вести счётчик, класть его в обе
      стороны в том же цикле, где уже выставляется `moved`.
- [ ] **Шаг 6.** Прогон — зелено.
- [ ] **Шаг 7.** `export-bindings`, `doc/features/F-203-move-pairing.md`.
- [ ] **Шаг 8.** Коммит: `feat(m7): pair the two ends of a moved block`

## Задача 7 · Перемещение внутри файла и между файлами (пункт 17)

`detect_moves` работает над одним `FileDiff`, поэтому перемещение между файлами
сейчас не находится вовсе.

**Файлы:** правка `crates/diff_engine/src/moves.rs`, `src/lib.rs`, `tests/moves.rs`.

**Интерфейс:**
- `pub enum MoveScope { WithinFile, AcrossFiles }`, `#[serde(default)]`.
- `pub fn detect_moves_across(diffs: &mut [(String, FileDiff)])` — второй проход
  поверх пачки из задачи 3; одиночный `detect_moves` остаётся как есть.

- [ ] **Шаг 1.** Тест `a_block_moved_inside_one_file_is_scoped_within_file`.
- [ ] **Шаг 2.** Тест `a_block_moved_between_two_files_is_scoped_across_files`.
- [ ] **Шаг 3.** Тест `a_cross_file_move_pairs_the_two_files_by_identifier`.
- [ ] **Шаг 4.** Прогон — падает.
- [ ] **Шаг 5.** Реализация: собрать удаления и вставки по всей пачке, применить тот
      же алгоритм совпадения, что и внутри файла; счётчик `move_id` общий на пачку,
      чтобы идентификаторы не столкнулись между файлами.
- [ ] **Шаг 6.** Прогон — зелено.
- [ ] **Шаг 7.** `export-bindings`, `doc/features/F-204-cross-file-moves.md`.
- [ ] **Шаг 8.** Коммит: `feat(m7): tell a move inside a file from one between files`

## Задача 8 · Разделитель в `git_engine/src/lib.rs`

Отдельной задачей и отдельным коммитом, чтобы `master` увидел протокол одной
строкой в истории, а не внутри чужой фичи.

**Файлы:** правка `crates/git_engine/src/lib.rs` — только добавление в конец.

- [ ] **Шаг 1.** Дописать в конец файла, слово в слово как в остальных пяти:
      `// ─── everything below this line belongs to the diff-merge branch; master appends above ───`
- [ ] **Шаг 2.** `cargo check -p git_engine`.
- [ ] **Шаг 3.** Дописать строку в таблицу общих файлов в
      [2026-09-21-split.md §1](2026-09-21-split.md) и поправить там же «в четырёх
      файлах» на «в семи» — сейчас текст говорит «четырёх», а таблица перечисляет шесть.
- [ ] **Шаг 4.** Коммит: `docs: give git_engine's lib.rs the branch divider too`

## Задача 9 · `.mailmap` (пункт 30)

**Файлы:** правка `crates/git_engine/src/blame.rs`, `crates/git_engine/src/lib.rs`
(ниже разделителя), `crates/git_engine/tests/blame.rs`.

**Проверить до кода:** `gix` 0.87 умеет `.mailmap` сам — открыть docs.rs на
`gix::Repository::open_mailmap` и на поле `mailmap` в опциях blame. Если умеет,
задача сводится к включению, а не к своему парсеру.

- [ ] **Шаг 1.** Фикстура: репозиторий, где один автор коммитит под двумя почтами,
      плюс `.mailmap`, сводящий их к одной. `fast-import` тут не подходит, если
      понадобится reflog, — но он не нужен, так что подходит (R-56).
- [ ] **Шаг 2.** Тест `two_spellings_of_one_author_collapse_to_the_canonical_name`.
- [ ] **Шаг 3.** Тест `a_repository_without_a_mailmap_is_unaffected`.
- [ ] **Шаг 4.** Прогон — падает.
- [ ] **Шаг 5.** Реализация в `blame()`.
- [ ] **Шаг 6.** Прогон — зелено.
- [ ] **Шаг 7.** `doc/features/F-205-mailmap.md`.
- [ ] **Шаг 8.** Коммит: `feat(m8): canonicalise blame authors through .mailmap`

## Задача 10 · `.git-blame-ignore-revs` (пункт 31)

**Файлы:** правка `crates/git_engine/src/blame.rs`, `tests/blame.rs`.

- [ ] **Шаг 1.** Фикстура: три коммита, средний — косметический (переотступовка),
      его OID записан в `.git-blame-ignore-revs`.
- [ ] **Шаг 2.** Тест `a_cosmetic_commit_listed_in_the_ignore_file_is_skipped`:
      строка приписана коммиту **до** косметического.
- [ ] **Шаг 3.** Тест `a_missing_ignore_file_is_not_an_error`.
- [ ] **Шаг 4.** Тест `an_unparsable_oid_in_the_ignore_file_is_not_an_error` —
      мусор в файле не должен ронять blame целиком.
- [ ] **Шаг 5.** Прогон — падает.
- [ ] **Шаг 6.** Реализация: прочитать файл, разобрать OID по строкам, комментарии
      с `#` пропустить, передать в опции blame `gix`.
- [ ] **Шаг 7.** Прогон — зелено.
- [ ] **Шаг 8.** `doc/features/F-206-blame-ignore-revs.md`.
- [ ] **Шаг 9.** Коммит: `feat(m8): honour .git-blame-ignore-revs in blame`

## Задача 11 · Состояние файла до выбранного коммита (пункт 32)

`blob_at(rev, path)` уже есть. Нужен разбор `<oid>^` и корректный отказ на корневом
коммите, у которого родителя нет.

**Файлы:** правка `crates/git_engine/src/blame.rs`, `tests/blame.rs`.

`blobs.rs` — территория `master`, поэтому метод `file_before(&self, oid: &str,
path: &str)` живёт в `blame.rs` и вызывает уже существующий `blob_at` оттуда.

- [ ] **Шаг 1.** Тест `the_state_before_a_commit_is_its_parents_version`.
- [ ] **Шаг 2.** Тест `the_state_before_the_root_commit_is_empty_rather_than_an_error`.
- [ ] **Шаг 3.** Тест `a_file_added_by_the_commit_has_no_state_before_it`.
- [ ] **Шаг 4.** Прогон — падает.
- [ ] **Шаг 5.** Реализация.
- [ ] **Шаг 6.** Прогон — зелено.
- [ ] **Шаг 7.** `export-bindings`, `doc/04-ipc-contract.md`,
      `doc/features/F-207-state-before-commit.md`.
- [ ] **Шаг 8.** Коммит: `feat(m8): open a file as it was before a commit`

## Задача 12 · Investigate: бэкенд (пункты 33, 34, 35)

Три пункта одним заходом: запуск поверх `git log -L`, отслеживание переименований и
хронология с diff по каждой правке — это один и тот же вызов и один разбор вывода.

**Файлы:** правка `crates/git_engine/src/find.rs`, `crates/git_engine/src/lib.rs`
(ниже разделителя); создать `crates/git_engine/tests/investigate.rs`;
правка `crates/app_state/src/lib.rs` и обоих файлов `src-tauri` ниже разделителя.

**Почему CLI, а не `gix`:** `git log -L` — единственная реализация отслеживания
диапазона через правки и переименования. Читающая операция, но своего аналога в
`gix` нет; это отступление от правила «чтение через gix» и его надо записать в
риски как R-101 ([03-git-semantics.md](../03-git-semantics.md)).

**Интерфейс:**
```rust
pub struct InvestigationStep {
    pub oid: String,
    pub summary: String,
    pub author: String,
    pub timestamp: i64,
    pub path: String,      // путь на момент этого коммита: меняется при переименовании
    pub diff: String,      // кусок unified diff по диапазону
}
pub fn investigate(&self, path: &str, from: u32, to: u32, limit: usize)
    -> Result<Vec<InvestigationStep>>;
```

- [ ] **Шаг 1.** Тест `a_range_reports_the_commits_that_touched_it` — хронология,
      новые первыми.
- [ ] **Шаг 2.** Тест `a_commit_that_did_not_touch_the_range_is_absent`.
- [ ] **Шаг 3.** Тест `each_step_carries_the_diff_of_that_edit` (пункт 35).
- [ ] **Шаг 4.** Тест `the_range_is_followed_across_a_rename` (пункт 34): фикстура с
      `git mv`, шаг до переименования несёт старый путь.
- [ ] **Шаг 5.** Тест `an_out_of_range_line_is_a_typed_error`, не паника.
- [ ] **Шаг 6.** Прогон — падает.
- [ ] **Шаг 7.** Реализация через `run_git` с
      `["log", "-L", "<from>,<to>:<path>", "--follow", "-z", ...]` и `LC_ALL=C`.
      Аргументы — массивом, `GIT_TERMINAL_PROMPT=0`, вывод не усекать (INV-05).
      Точный набор флагов подобрать по `git log --help` установленной версии,
      а не по памяти: формат `-L` менялся.
- [ ] **Шаг 8.** Прогон — зелено.
- [ ] **Шаг 9.** `R-101` в `doc/12-risks.md` ниже разделителя: почему Investigate
      идёт через CLI.
- [ ] **Шаг 10.** `export-bindings`, `doc/04-ipc-contract.md`,
      `doc/features/F-208-investigate.md`.
- [ ] **Шаг 11.** Коммит: `feat(m8): trace a line range through history`

## Задача 13 · Подтверждение уже закрытых пунктов

Не код, а доказательство. Пункты 9, 10, 13, 18 закрыты до начала ветки — это надо
зафиксировать, иначе при ревью они выглядят пропущенными.

**Файлы:** создать `doc/features/F-209-…` не нужно — фичи уже описаны. Правка
`doc/iterations/2026-09-21-diff-merge-rust.md` (этот файл), раздел «Что уже сделано».

- [ ] **Шаг 1.** Прогнать поимённо тесты из таблицы и вставить сюда вывод nextest.
- [ ] **Шаг 2.** Коммит: `docs: record which of the branch's steps were already green`

---

## Порядок и зависимости

```
1 (R-100)  ─┐
2 (бюджеты) ─┴─> 3 (пачка) ─> 4 (отмена)
5 (no-newline) ─ независима
6 (парный id) ─> 7 (scope, нужна пачка из 3)
8 (разделитель) ─> 9 (mailmap) ─> 10 (ignore-revs)
                └─> 11 (до коммита)
                └─> 12 (Investigate)
13 — в конце
```

## Итог захода

Сделано 21 сентября 2026, база — `5f9494b` (после `git rebase master`, когда влилась
декомпозиция `App.svelte`).

| Пункт | Коммит | Чем закрыт |
|---|---|---|
| 1 | `91fcae2` | R-100: сравнение по семи требованиям, R-10 закрыт |
| 12 | `ce1db51` | `tests/budget.rs`, четыре замера; R-101 про недостижимую строку §9 |
| 7 | `b7ff128` | `diff_many` через `rayon`, `AppState::diff_files`, команда `diff_files` |
| 8 | `a3565fe` | `DiffBatch::{Ready,Superseded}`, номер запроса; R-102 |
| 11 | `0f03b42` | `noNewline` в `DiffRow`; попутно найдено, что появление финального перевода строки показывалось как «без изменений» |
| 14 | `de4726d` | `moveId` на обоих концах перемещения |
| 17 | `d9d47da` | `MoveScope`, `link_moves_across_files`, сквозная нумерация по пачке |
| — | `dc8b220` | разделитель в `git_engine/src/lib.rs`, седьмой общий файл |
| 30 | `c6047d4` | `.mailmap` через `repo.open_mailmap()`, поле `email` в `BlameLine` |
| 31 | `694a1cc` | `.git-blame-ignore-revs` поверх blame; R-103 про сопоставление по номеру строки |
| 32 | `60ce6b7` | `file_before`, команда `file_before` |
| 33, 34, 35 | `c7628a2` | `investigate` поверх `git log -L`; R-104 про чтение через CLI |

Пункты 9, 10, 13 и 18 были закрыты до начала ветки. Проверено поимённым прогоном:
`a_file_that_differs_only_in_line_endings_is_not_a_rewrite`,
`line_endings_are_normalized_before_comparing`,
`a_file_with_mixed_line_endings_still_diffs_line_by_line`,
`mixed_line_endings_are_reported_as_mixed`,
`offsets_are_utf16_units_so_javascript_can_slice_directly`,
`an_emoji_replaced_by_an_emoji_keeps_its_boundaries`,
`a_block_shorter_than_the_threshold_is_not_a_move`,
`indentation_alone_does_not_hide_a_move`,
`all_three_sides_are_readable_while_the_merge_is_unresolved`.

Замеры бюджетов [§9](../08-diff-engine.md):

| Что | Измерено | Бюджет |
|---|---|---|
| Файл 5 000 строк | 2 мс | 50 мс |
| Файл 100 000 строк | 148 мс | 1000 мс |
| Пачка 500 файлов | 7 мс | 2000 мс |

### Что осталось за `master`

1. `.claude/worktrees/` не в `.gitignore`: worktree ветки виден в основном чекауте как
   неотслеживаемый каталог.
2. Восемь мёртвых зависимостей CodeMirror в `frontend/package.json` ([R-100](../12-risks.md)).
3. Разделитель в `crates/app_state/src/lib.rs` стоит после `mod tests` ([R-102](../12-risks.md)).
4. `AppState::diff_file` повторяет конвейер, который теперь живёт в `diff_engine::diff_one`;
   свести их нельзя, пока метод выше разделителя.
5. `doc/04-ipc-contract.md` разделителя не имеет; ветка правила подраздел `### Diff`.

### Что осталось ветке

Пункты 2–6, 15, 16, 19–29, 36 — фронтенд. Точка входа готова:
`components/panels/DiffPanel.svelte` пришёл с `master` и разводит по четырём компонентам
ветки. Бэкенд под них уже лежит: `moveId`, `moveScope`, `noNewline`, `diffFiles`,
`investigate`, `fileBefore`.

## Проверка перед каждым коммитом

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run -p diff_engine -p git_engine -p app_state
```

Перед push — весь набор: `cargo nextest run --workspace --exclude cogit`.
