# Cogit — Контракт IPC

> Граница между Rust и Svelte. Любое изменение команды, события или DTO
> обязано быть отражено здесь **в том же коммите**.

## 1. Три механизма

| Механизм | Направление | Когда применять |
|---|---|---|
| `invoke` (команда) | UI → Rust → UI | Запрос/ответ, результат до ~500 элементов |
| `Channel<T>` | Rust → UI, потоком | Длинные списки, прогресс долгих операций ([INV-02](01-architecture.md#inv-02)) |
| `Event` | Rust → UI, широковещательно | Изменения, которые UI не запрашивал: ФС, завершение фоновой задачи |

Обратного канала «UI → Rust потоком» нет и не нужно: пользователь не генерирует потоки данных.

## 2. Генерация типов

Все DTO помечаются `#[derive(serde::Serialize, specta::Type)]` (и `Deserialize` для входных).
Команды помечаются `#[tauri::command]` + `#[specta::specta]`.

Сборка биндингов происходит в `src-tauri`, результат — `frontend/src/lib/ipc/bindings.ts`.
Файл **коммитится** в репозиторий (чтобы `npm run check` работал без предварительной сборки Rust),
но **правится только генератором** ([INV-10](01-architecture.md#inv-10)).

Фронтенд импортирует команды из биндингов, а не вызывает `invoke` со строковым именем:

```ts
import { commands } from "$lib/ipc/bindings";
const res = await commands.openRepository(path);   // тип известен компилятору
```

Проверка актуальности биндингов входит в pre-commit хук: если генерация даёт diff — коммит
отклоняется с подсказкой перегенерировать.

## 3. Соглашения

| Тема | Правило |
|---|---|
| Именование команд | `snake_case` в Rust → `camelCase` в TS (делает specta) |
| Именование полей DTO | **Каждый** DTO помечается `#[serde(rename_all = "camelCase")]`. specta следует за serde, а не переименовывает сам: без этого атрибута поля приедут в TS как `log_path`, и UI будет молча читать `undefined` |
| Идентификатор репозитория | `RepoId` — непрозрачный `u32`, выдаётся `app_state`. Пути в IPC не гоняем |
| OID | hex-строка, поле называется `oid` |
| Пути файлов | относительно корня репозитория, разделитель `/` **на всех платформах** |
| Время | Unix timestamp в секундах (`i64`) + отдельное поле смещения зоны в минутах |
| Ошибки | Команды возвращают `Result<T, GitError>` (`git_engine::GitError`); во фронтенде он оборачивается в класс `CogitError` (`$lib/ipc`) |
| Отсутствие значения | `Option<T>` → `T \| null`, никаких «пустых строк вместо null» |

### Тип ошибки

```rust
// crates/git_engine/src/error.rs
#[derive(Debug, thiserror::Error, Serialize, specta::Type)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
pub enum GitError {
    #[error(transparent)]
    Command(Box<GitCommandError>),        // "command": ошибка CLI, Git Error Dialog
    RepoNotFound(String),                 // "repoNotFound"
    InvalidState(String),                 // "invalidState": detached HEAD там, где нужна ветка
    Io(String),                           // "io"
    Internal(String),                     // "internal"
    ModuleUnavailable(ModuleProblem),     // "moduleUnavailable": сабмодуль не открыть, с причиной
    ConfigInvalid(ConfigProblem),         // "configInvalid": git отверг текст конфига, ничего не записано
}
```

Размеченное объединение, а не строка: фронтенду нужно **различать** ошибку CLI (открыть
специальный диалог с сырым выводом) и остальные (показать тост).

## 4. Команды

Ниже — контракт. Реализуются по модулям; колонка «Модуль» указывает, когда команда появляется.

> **Сверка 2026-09-24:** таблица отстаёт от кода — около 50 зарегистрированных команд в ней
> нет (`repo_status`, `stashes`, `flow_*`, `graph_window`, `graph_row_of`, `safety_log`,
> `undo_entry`, `conflict_*` и др.), часть строк описывает команды, которых нет
> (`list_repositories`, `repo_state`, `list_refs`, `list_stashes`, `list_reflog`,
> `diff_working_tree`, `merge_conflict`, `stage_hunk`, `open_in_explorer`), у части
> расходятся сигнатуры (`fetch`, `pull`, `push`, `open_in_terminal`, `stage_selection`).
> Источник истины по именам и типам — `collect_commands!` в `src-tauri/src/lib.rs` и
> сгенерированный `frontend/src/lib/ipc/bindings.ts`; таблица — по назначению команд.

### Репозитории

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `open_repository` | `path: String` | `RepoSummary`; в нём `tagGroupSeparator` — `cogit.tagGroupSeparator` из конфига репозитория, `/` если не задан, `""` — теги без папок; перечитывается при каждом открытии и обновлении (#11) | M1 |
| `close_repository` | `repo: RepoId` | `()` | M1 |
| `list_repositories` | — | `Vec<RepoEntry>` | M3 |
| `repo_state` | `repo: RepoId` | `RepoState` — `clean | detachedHead { oid } | merging | rebasing | cherryPicking | reverting | bisecting | applyingPatches | empty | bare`; `applyingPatches` — `git am`, остановленный на патче (`rebase-apply/applying`) | M1 |
| `repositories` | — | `Result<Vec<RepoOverview { repo, name, root, branch, ahead, behind, dirty, missing, state: RepoState }>>`; `state` — для меток `<merging>`/`<detached>` в дереве (#22). Читается в `spawn_blocking`, поэтому `Result` | M3 |
| `list_submodules` | `repo: RepoId` | `Vec<Submodule>` | M3 |
| `worktrees` | `repo: RepoId` | `Vec<WorktreeEntry { path, name, branch, head, isMain, isCurrent, locked, missing, dirty }>`; из linked-ворктри основной — всё равно основной (R-184); у `missing` ветка и HEAD читаются из записи `.git/worktrees/<id>/HEAD` (R-241) | M3 |
| `open_worktree` | `owner: RepoId`, `path` — существующий ворктри владельца | `RepoSummary`, в списке Repositories не появляется; чужая папка — `InvalidState` | M3 |
| `add_worktree` | `repo`, `path`, `branch`, `create`, `base: Option<String>` — откуда новая ветка, по умолчанию HEAD | `()` | M3 |
| `remove_worktree` | `repo`, `path`, `force` | `()`; при `force` изменения сначала в stash, в журнале — Undo (INV-12) | M3 |
| `worktree_changes` | `repo`, `path` | `Vec<FileEntry>` — незакоммиченное в этом ворктри, для подтверждения Remove | M3 |
| `prune_worktrees` | `repo` | `()` — `git worktree prune`, все устаревшие | M3 |
| `prune_worktree` | `repo`, `path` | `()` — одна регистрация; папка на месте — `InvalidState` | M3 |
| `repair_worktree` | `repo`, `path` — где папка теперь | `()` — `git worktree repair <path>`, затем `git update-index -q --refresh` в починенном и в текущем worktree; в очереди с подписью `Repairing worktree <name>` | M3 |
| `lock_worktree` / `unlock_worktree` | `repo`, `path`, `reason: Option<String>` (только lock) | `()` | M3 |

### Хуки и пресеты (M10)

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `list_presets` | `repo: RepoId` | `Vec<PresetStatus>` | M10 |
| `export_preset` | `repo, hook, id, name, description` | `()` | M10 |
| `remove_preset` | `id: String` | `()` | M10 |

`list_presets` теперь берёт репозиторий: `missingConfig` считается против его рабочего
дерева, иначе предупреждение о `.clang-format` не с чем сверять. `user: true` отличает
пресет, сохранённый пользователем, — только такой можно удалить.

Обход хука (`--no-verify`) идёт в журнал обычной записью `GitOutput` с кодом 0 и строкой
на `stderr`: панель Output помечает такие как предупреждения.

### Слияние (M7, M2)

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `merge_preview` | `repo, path` | `Vec<Region>` | M7 |
| `open_merge_window` | `url, title` | `()` | M2 |
| `merge_resolved` | `repo, path` | `()` | M2 |

Окно слияния — второй входной файл `merge.html`, как и окно сравнения: параметры идут в
URL, чтобы окно пережило перезагрузку вебвью. Записав результат, оно зовёт
`merge_resolved`, а тот шлёт событие `merge-resolved` всем окнам; главное закрывает
панель конфликта и перечитывает состояние.

### Окно Blame (M8, M2)

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `blame` | `repo, path, rev` | `Vec<BlameLine>` | M8 |
| `open_blame_window` | `repo, path, rev` — любая ревизия, `HEAD` тоже | `()` | M2 |
| `file_revisions` | `repo, path, rev` | `Vec<CommitRow>` — коммиты до `rev`, менявшие файл, новые сверху; не больше 500 | M8 |
| `line_history` | `repo, path, rev, line` — строка с 1 | `Vec<LineVersion { oid, summary, author, email, timestamp, path, line, text }>`, новые сверху; не больше 200 | M8 |

Blame открывается только отдельным окном (`blame.html`). `open_blame_window` сам разрешает
`rev` в коммит: заголовок окна — `<файл> - Blame of <путь>@<короткий хеш>`, в URL уходит
полный хеш, так что окно после перезагрузки вебвью показывает ту же версию.

`line_history` — история одной строки: коммиты, менявшие её, и сама строка в каждой версии
(`text`, номер `line`, путь `path` на момент коммита — он меняется при переименовании). Идёт
через `git log -L`, как `investigate` ([R-202](12-risks.md)). `file_revisions` — список
версий для `View Commit` и `Highlight: Changes Since`, обход через `gix` без следования за
переименованием.

У окна своё меню. Id его пунктов — `child:<метка окна>:<действие>`; обработчик меню
приложения узнаёт их по префиксу и не шлёт в `menu-command`. `close` закрывает окно в Rust,
остальные действия доходят только до этого окна DOM-событием `cogit-menu` (`detail` —
действие).

### Аватары (M14)

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `avatars` | `authors: Vec<Author>` | `Vec<AvatarRow>` | M14 |
| `set_avatars` | `enabled: bool` | `()` | M14 |

`avatars` возвращается **сразу** и отдаёт только то, что уже в кэше; заодно этот вызов
задаёт окно очереди — всё, чего в списке нет, из очереди выбрасывается. Пришедшая позже
картинка объявляется событием `avatar-ready { email }`, после чего фронтенд спрашивает
одного этого автора.

`AvatarRow` всегда несёт `initials` и `color`; `image` — `data:`-URL или `null`. Путь к
файлу в вебвью не уходит: у него нет доступа к каталогу кэша.

`set_avatars(false)` — это и выключение колонки, и отказ от сети: каталог кэша создаётся
только при `true`.

### История и граф

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `load_commits` | `repo, query: CommitQuery, channel: Channel<GraphChunk>` | `Vec<SkippedRef { name, reason }>` — отмеченные ссылки, не ставшие стартовой точкой; новый вызов останавливает предыдущий обход | M4 |
| `commit_details` | `repo, rev: String` | `CommitDetails` | M4 |
| `commit_files` | `repo, rev: String` | `Vec<FileEntry>` | M6 |

`search_commits` из первоначальной спеки **свёрнут в `load_commits`**: отфильтрованная
история — это тот же обход с более узким предикатом, и две команды означали бы две копии
логики стриминга. Пустой `CommitQuery` даёт полную историю.

```rust
pub struct CommitQuery {
    pub author: Option<String>,      // подстрока имени или e-mail, регистр не важен
    pub message: Option<String>,     // подстрока темы коммита
    pub oid_prefix: Option<String>,
    pub since: Option<i64>,          // секунды Unix, включительно
    pub until: Option<i64>,
    pub path: Option<String>,
    pub visible_refs: Option<Vec<String>>, // отмеченное в панели References
    pub long_link_rows: Option<u32>, // не фильтр: связи длиннее — обрубками (R-330)
}
```

Условия объединяются по **И**. Фильтр по пути сравнивает запись дерева с первым
родителем — так же, как `git log -- path` до отслеживания переименований, — и проверяется
последним, потому что стоит два обращения к дереву на каждого кандидата.

`visible_refs` стоит особняком: это не предикат по коммиту, а список вершин обхода.
`None` — все ссылки; `Some([])` — ни одной, то есть пустой граф; `Some([имена])` — обход
ровно от них. Каждый элемент разрешается через `rev_parse_single`, поэтому годятся и полное
имя ссылки, и `stash@{2}`, и голый OID потерянного коммита. Не разрешившийся элемент
пропускается: ветку могли удалить, пока панель была на экране.

**Отфильтрованный результат — плоский список без рёбер.** Родители совпавшего коммита
обычно отфильтрованы, и дорожки между выжившими утверждали бы родство, которого нет.
Сужение `visible_refs` — **исключение**: оно убирает вершины целиком, поэтому все предки
выживших вершин на месте и дорожки остаются правдивыми. Решает это `CommitQuery::filters_rows()`,
а не `is_empty()` ([R-51](12-risks.md)).

`long_link_rows` — тоже не предикат, а параметр раскладки этой загрузки: связь коммита с
родителем длиннее стольких строк рисуется двумя обрубками со стрелками, и её колонка
освобождается ([07-graph-rendering.md §3](07-graph-rendering.md#3-алгоритм-раскладки), R-330).
`None` и `0` — все связи целиком. `filters_rows()` и `is_empty()` его не видят. Чтобы решить,
длинная ли связь, раскладке нужно заглянуть на столько коммитов вперёд, поэтому последние
строки обхода приходят в чанке с `isLast` и `total` отстаёт от числа обойдённых коммитов.

`rev` — любая ревизия в понимании `git rev-parse` (`HEAD`, `HEAD~2`, полный или сокращённый OID),
а не только OID: панель деталей использует то же поле, что и будущая строка перехода.

```rust
pub struct CommitDetails {
    pub oid: String,
    pub parents: Vec<String>,   // порядок Git: первый родитель — mainline
    pub summary: String,        // первая строка сообщения
    pub body: String,           // остальное, без хвостового перевода строки
    pub author: Signature,
    pub committer: Signature,
}

pub struct Signature {
    pub name: String,
    pub email: String,
    pub timestamp: i64,         // секунды Unix, в TS — number
    pub tz_offset_minutes: i32,
}

pub struct FileEntry {
    pub path: String,             // относительно корня, всегда через `/`
    pub old_path: Option<String>, // заполнен только для Renamed и Copied
    pub status: FileStatus,
}

pub enum FileStatus { Added, Modified, Deleted, Renamed, Copied }
```

`commit_files` сравнивает коммит **с первым родителем**, как это делает `git show`.
Корневой коммит сравнивается с пустым деревом, поэтому все его файлы — `Added`.
Отслеживание переименований включено явно (`track_rewrites`), а не берётся из конфига
репозитория: результат не должен зависеть от настроек пользователя.

### Ссылки

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `list_refs` | `repo` | `RefTree` | M5 |
| `list_stashes` | `repo` | `Vec<StashEntry>` | M5 |
| `stash_contents` | `repo, index: usize` | `Vec<FileEntry>` | M5 |
| `list_reflog` | `repo` | `Vec<ReflogEntry>` | M5 |
| `ref_dates` | `repo` | `Vec<RefDate { fullName, timestamp }>` — дата вершины каждой ветки (локальной и remote) и тега в секундах Unix: у аннотированного тега — дата тега, иначе — committer-дата коммита; зовётся только при сортировке Branches по дате (#20) | M5 |

### Diff

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `diff_file` | `repo, spec: DiffSpec, path, options: DiffOptions` | `FileDiff` | M7 |
| `diff_files` | `repo, spec: DiffSpec, paths: string[], options: DiffOptions, request` | `DiffBatch` | M7 |
| `diff_working_tree` | `repo, path` | `FileDiff` | M7 |
| `merge_conflict` | `repo, path` | `ThreeWayDiff` | M7 |
| `file_before` | `repo, oid, path` | `string \| null` | M8 |
| `investigate` | `repo, path, from, to, limit` | `InvestigationStep[]` | M8 |
| `investigate_log` | `repo, path, rev: string \| null, follow, on_chunk: Channel<FileRevision[]>` | `number` (всего) | M8 |
| `investigate_blame` | `repo, path, rev: string \| null, ignore_whitespace, on_chunk: Channel<BlameChunk>` | `number` (строк) | M8 |
| `origin_candidates` | `repo, query: OriginQuery, on_event: Channel<OriginEvent>` | — | M8 |
| `discard_selection` | `repo, request: PatchRequest` | — | M6 |

`discard_selection` — обратная сторона `stage_selection`: тот же `PatchRequest`, но патч
накладывается на **рабочее дерево**, а не на индекс. Деструктивно; пишется в журнал
безопасности как неоткатываемое ([R-106](12-risks.md)), и вьюер обязан переспросить.

Патч выделенных строк собирается в Rust под направление наложения. Вперёд (Stage) он
совпадает со старой стороной диффа: невыбранное удаление остаётся контекстом, невыбранная
вставка выпадает. Назад (Unstage, Discard) — с новой: невыбранная вставка становится
контекстом, невыбранное удаление выпадает. Маркер `\ No newline at end of file` ставится
под каждой строкой, у которой на своей стороне нет перевода строки (`noNewline` у строки
диффа, в том числе у контекста). `/dev/null` в заголовке — только если файла на этой
стороне нет и выделение забирает его целиком.

`investigate` — история диапазона строк, а не файла: `InvestigationStep { oid, summary,
author, email, timestamp, path, diff }`, новые сверху. `path` — имя файла **на момент того
коммита**, оно меняется при переименовании. Идёт через `git log -L`, а не через `gix`
([R-104](12-risks.md)); `limit` зажимается в 1…1000. Окно Investigate (#15) им больше
не пользуется — команда оставлена для будущего фильтра Navigation по выделенным строкам
([R-282](12-risks.md)).

Окно Investigate ([R-280](12-risks.md), [R-281](12-risks.md)):

- `investigate_log` — коммиты, менявшие файл, новые сверху, от `rev` (HEAD, если `null`);
  `follow` — через переименования (`git log --follow`). `FileRevision { oid, parents,
  summary, author, email, timestamp, path, previousPath, change }`: `path` — имя файла в
  этом коммите, `previousPath` — прежнее имя у переименования и копии. Чанки по 200, не
  больше 10 000 записей; возвращает общее число — фронтенд ждёт и его, и последний чанк.
- `investigate_blame` — файл на `rev` (рабочая копия, если `null`), `git blame --porcelain
  -M -C -C` (`-w` при `ignore_whitespace`). Первый чанк — `header { commits, sources }`,
  дальше `lines { lines }` по 200. `OriginLine { line, text, source, origLine, change }`:
  `source` — индекс в `sources` (`BlameSource { commit, path, previous }` — коммит, файл,
  в котором он написал строку, и тот же файл шагом раньше), `origLine` — номер строки в
  той версии, `change` — `added` / `modified`. `BlameCommit { …, merge, boundary,
  uncommitted }`; незакоммиченные строки — коммит из сорока нулей.
- `origin_candidates` — фоновый поиск происхождения блока. `OriginQuery { commit, path,
  from, to, line, previous }` — блок, внесённый одним коммитом. События: `started { id }`
  (для `cancel_operation`), затем `done { report }` или `cancelled`. `OriginReport {
  candidates, best }`; `OriginCandidate { kind: appeared | modified | moved | copied, rev,
  path, from, to, score (проценты), likelihood: high | medium | low, deeper, block,
  source }`, где `deeper { rev, path, line }` — куда ведёт Go Deeper (`null` — глубже
  некуда), `block` и `source` — построчное сравнение (`same` / `changed` / `missing`) для
  перспективы Origins.

`diff_files` — та же работа, что `diff_file`, но сразу по всем файлам коммита: чтение
объектов последовательное, само сравнение параллельное через `rayon` внутри
`spawn_blocking` ([INV-01](01-architecture.md), [§9 08-diff-engine.md](08-diff-engine.md)).

`DiffBatch` — объединение по `kind`: `ready` со списком `FileDiffEntry { path, diff }`
**в порядке запроса**, либо `superseded`. Путь, которого нет ни на одной стороне, даёт
ошибку на всю пачку, а не тихо выпадает из ответа.

`request` — номер, который фронтенд обязан увеличивать при каждой смене выбранного
коммита. Пачка с номером меньше уже виденного не считается вовсе, а запущенная проверяет
между файлами, не пришёл ли номер новее, и в этом случае бросает работу и отвечает
`superseded` ([R-102](12-risks.md)). Ответ `superseded` — не ошибка: он означает, что
пользователь уже смотрит на другой коммит, и его надо молча игнорировать.

`DiffSpec` описывает, что с чем сравнивается: `WorkTreeVsIndex`, `IndexVsHead`,
`CommitVsParent { oid }`, `CommitVsCommit { a, b }`, `StashVsParent { index }`.
Реализованы `CommitVsParent`, `CommitVsCommit`, `WorkTreeVsIndex` и `IndexVsHead`;
`StashVsParent` придёт с M5.

`FileDiff` — размеченное объединение по полю `kind`: `text`, `eolOnly`, `binary`,
`image`, `tooLarge`, `unchanged`. Вариант `text` несёт ханки, сведения об окончаниях
строк, флаг `lossyEncoding`, подсказку грамматики для Lezer и `oldTotal`/`newTotal` —
число строк каждой стороны: по ним панель знает, сколько строк скрыто под последним ханком
(#16, R-271).

`DiffOptions` — `algorithm`, `contextLines`, `ignoreWhitespace`,
`wordDiff`, `detectMoves`. Значения приходят из настроек (F-078); `detectMoves`
управляет пометкой перемещённых блоков, остальные — самим сравнением.

```rust
pub struct Hunk {
    pub old_start: u32,   // 1-based; 0, когда старой стороны нет
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub header: String,   // "@@ -7,7 +7,7 @@"
    pub rows: Vec<DiffRow>,
}

pub enum DiffRow {
    Context { old: u32, new: u32, text: String },
    Delete  { old: u32, text: String, inline: Vec<(u32, u32)> },
    Insert  { new: u32, text: String, inline: Vec<(u32, u32)> },
    Collapsed { count: u32 },
}
```

Нумерация строк в `DiffRow` — 1-based и относится к той стороне, к которой принадлежит
строка. Текст приходит **без** завершающего перевода строки: он одинаков для всех строк
и только мешает отрисовке.

**Имя параметра команды становится идентификатором в TypeScript.** Параметр `switch: bool`
сгенерировал `(repo, name, start, switch)` — синтаксическая ошибка, потому что `switch` —
зарезервированное слово JS. Ломается весь файл биндингов, а не одна строка. Проверено на
`create_branch`, переименован в `switch_to`.

`rename_all_fields = "camelCase"` обязателен на enum-ах со структурными вариантами:
`rename_all` переименовывает только имена вариантов, а поля внутри них утекают в
snake_case и читаются на фронтенде как `undefined`.

### Мутации

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `stage_paths` / `unstage_paths` | `repo, paths: Vec<String>` | `()` | M6 |
| `worktree_files` | `repo` | `WorktreeFiles` | M6 |
| `stage_hunk` | `repo, patch: String` | `()` | M6 |
| `discard_paths` | `repo, paths` | `()` | M6 |
| `commit` | `repo, request: CommitRequest { message, amend, noVerify }` | `String` (oid) | M6 |
| `checkout` | `repo, target: CheckoutTarget` | `()` | M5 |
| `create_branch` / `delete_branch` | `repo, ...` | `()` | M5 |
| `merge` / `rebase` / `cherry_pick` / `revert` | `repo, ...` | `()` | M5 |
| `stash_push` / `apply` / `pop` / `drop` | `repo, ...` | `()` | M5 |
| `stash_keeping_worktree` | `repo, message` | `()` — `git stash create` + `git stash store --message`: stash без очистки рабочей копии; untracked-файлы в него не входят; чистое дерево — `InvalidState` (R-212) | M5 |
| `stash_selection` | `repo, paths, message` | `()` — пустое `message` не передаётся в Git: stash получает его собственное `WIP on …` | M5 |
| `fetch` / `pull` / `push` | `repo, remote, refspec, channel: Channel<Progress>` | `()` | M1 |
| `undo_last` | `repo` | `UndoResult` | M5 |

Все мутации возвращают `Result<_, CogitError>` и при неуспехе CLI — вариант `GitCommand`.

### Поиск в панели Files

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `commit_tree_files` | `repo`, `rev` | `Vec<String>` — все файлы дерева коммита, отсортированы; подмодуль — одна запись | M6 |
| `search_file_contents` | `repo`, `query`, `is_regex`, `scope`, `Channel<SearchChunk>` | `()` | M6 |
| `list_submodules` | `repo`, `parent` (пусто — верхний уровень) | `Vec<Submodule>` | M3 |
| `open_submodule` | `owner: RepoId`, `key` — путь узла от владельца дерева | `RepoSummary`; отказ — `GitError::ModuleUnavailable(ModuleProblem)` | M3 |
| `repository_health` | `repo` | `Vec<HealthFinding { module, issue }>` — репозиторий и все подмодули; `issue`: `ignoreCaseMismatch`, `danglingModule`, `danglingWorktree`, `missingModuleCommit { commit }` (R-179) | M3 |
| `read_git_config` | `repo: Option<RepoId>`, `scope: repository \| user` | `ConfigFile { path, text, crlf, exists }` | M3 |
| `write_git_config` | `repo`, `scope`, `text`, `crlf` | `()`; отказ git — `GitError::ConfigInvalid { line, message }` | M3 |
| `cancel_operation` | `id` | `bool` — `false`, если уже закончилась | — |
| `list_operations` | — | `Vec<Operation>` — всё, что в очереди и в работе | — |

`list_all_repo_files` отвечает на «где этот файл», а не «что изменилось»: панель ищет файл
и тогда, когда с ним ничего не происходило.

`commit_tree_files` нужен переключателю `Unchanged` в коммите из истории (#3): список
запрашивается, только пока переключатель включён, и не чаще раза на коммит.

`search_file_contents` стримит через `Channel`, а не возвращает список: совпадений бывает
больше пятисот, и правило про порции ([03-git-semantics.md](03-git-semantics.md), INV-10)
распространяется и на них. Порция — сто совпадений.

```ts
type SearchChunk =
  | { kind: "started";  id: number }
  | { kind: "matches";  matches: ContentMatch[] }
  | { kind: "done";     total: number; cancelled: boolean }
```

`started` приходит **первым** и несёт `id`: панель должна уметь отменить поиск раньше, чем
получит ответ, потому что пользователь продолжает печатать. Файлы больше 2 МБ и бинарные
(NUL в первых 8000 байтах, правило git) не открываются вовсе.

`list_submodules` перечисляет **один уровень**. Репозиторий с девятью сабмодулями, у каждого
свои, стоит одного обхода на уровень, а дереву нужен только раскрытый узел.

`Submodule.state` — `notInitialised | inSync | ahead | behind | diverged | unknown`, с
`ahead`/`behind` — числом коммитов по обе стороны общего предка (R-153). `unknown` —
записанного коммита в подмодуле нет, и положение не угадывается. `Submodule.repoState` —
`RepoState` его собственного репозитория (`null`, пока он не выписан): дерево ставит на узел
метку операции, остановленной внутри подмодуля (#22).

`open_submodule` принимает ключ, а не путь: путь из ключа собирает бэкенд тем же
`module_root`, что и `list_submodules`, и открывает ровно там, без поиска вверх (R-149).

Каждая строка несёт `nested` — есть ли у сабмодуля свои сабмодули. Это проверка
существования `<path>/.gitmodules`, сделанная там же, при перечислении: без неё дерево
не знает, рисовать ли треугольник, и рисует его всем (R-148).

`cancel_operation` останавливает **чтения**, не мутации: операция, брошенная на середине,
оставила бы репозиторий в состоянии, которого никто не просил. При закрытии приложения все
идущие чтения гасятся автоматически.

### Remote ▸ Submodule, Subtree, LFS и Repository ▸ Settings (#42, #45, #46)

Команды — в `src-tauri/src/commands/remote_ops.rs`, логика — в `git_engine` (`module_ops.rs`,
`subtrees.rs`, `lfs.rs`, `repo_settings.rs`). Мутации идут через очередь репозитория.

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `submodule_op` | `repo`, `op: initialize \| synchronize \| reset \| deactivate \| deinit \| unregister`, `paths` (пусто — все, только для `initialize` и `synchronize`) | `()`; путь не сабмодуль — `InvalidState` до запуска git | M3 |
| `add_submodule` | `repo`, `url`, `path`, `branch: Option<String>` | `()` | M3 |
| `subtree_op` | `repo`, `op: SubtreeOp` — `{ kind: add, prefix, repository, reference, squash }`, `{ kind: merge, prefix, repository: string \| null, reference, squash }` (без `repository` — `git subtree merge`, с ним — `pull`), `{ kind: split, prefix, branch, rejoin }`, `{ kind: reset, prefix, reference }`, `{ kind: push, prefix, repository, reference }` | `()`; папка вне репозитория — `InvalidState` | M3 |
| `subtree_prefixes` | `repo` | `Vec<String>` — папки из строк `git-subtree-dir:` истории, которые ещё есть | M3 |
| `lfs_version` | — | `Option<String>` — `None`, если у git нет команды `lfs` | M3 |
| `lfs_op` | `repo`, `op: LfsOp` — `install` (`--local`), `{ track, pattern }`, `{ lock, paths }`, `{ unlock, paths }`, `prune` | `()` | M3 |
| `repo_settings` | `repo` | `Vec<RepoSetting { key, local, inherited }>` — ключи `REPO_SETTING_KEYS` по порядку; `local` — из конфига репозитория, `inherited` — из пользовательского и системного | M3 |
| `write_repo_settings` | `repo`, `changes: Vec<RepoSettingChange { key, value: string \| null }>` | `()`; `null` снимает ключ; ключ не из списка — `InvalidState`, и не пишется ничего | M3 |

`cogit.*` в `REPO_SETTING_KEYS` — ключи самого Cogit в `.git/config`: `cogit.tagGroupSeparator`
(разделитель папок тегов, по умолчанию `/`, пустая строка — без папок; читает дерево Branches) и
`cogit.initNewSubmodules` (после Pull инициализировать сабмодули, которых до него не было).

### Служебные

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `app_info` | — | `Result<AppInfo { version, debug_build, commit, dirty, built_at, repository, os: OsInfo, renderer, git, rustc, tauri, git_library, log_path, log_dir, settings_path, displays: DisplayInfo[] }>`; async, вне главного потока (R-175) | M0, M2 |
| `open_third_party_licences` | `frontend: string \| null` | `Result<()>`: пишет список лицензий (крейты из `build.rs` + пакеты из сборки Vite) во временный файл и открывает его системой (R-173) | M2 |
| `read_settings` | — | `String` — весь документ настроек как текст JSON | M8 |
| `write_setting` | `key`, `value` (текст JSON) | `()` | M8 |
| `command_log` | `limit` | `Vec<CommandLogEntry>` | M2 |
| `open_in_explorer` / `open_in_terminal` | `path` | `()` | M3 |
| `set_menu_state` | `disabled: Vec<String>` | `()` | M2 |
| `report_memory` | `RendererMemory { usedHeapKib, totalHeapKib, limitKib, domNodes, listeners, caches }` | `()` | — |

`report_memory` шлёт вебвью раз в десять секунд и **только в отладочной сборке**; строка
ложится в профиль как `kind=mem` ([14-profiling.md](14-profiling.md)). Величины идут в KiB,
а не в байтах: specta запрещает 64-битные целые в типах IPC, а 32 бита KiB — это четыре
терабайта.

### Хуки и учётные данные

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `list_hooks` | `repo` | `HookOverview` | M10 |
| `read_hook` / `write_hook` | `repo, name[, body]` | `String` / `()` | M10 |
| `set_hook_enabled` | `repo, name, enabled` | `()` | M10 |
| `use_hooks_path` | `repo, path` | `()` | M10 |
| `run_hook` | `repo, name` | `HookRun` | M10 |

### Хирургия коммитов и rebase

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `rollback_to` | `repo, rev, paths` | `()` | M12 |
| `split_off` | `repo, rev, paths, message, splitFirst` | `()` | M12 |
| `is_published` | `repo, rev` | `bool` | M12 |
| `is_merged_into_head` | `repo, rev` | `bool` — HEAD уже содержит `rev` (равен или предок); через `gix`, спрашивается тулбаром при каждой смене выбора (R-210) | M2 |
| `delete_merged_branches` | `repo` | `Vec<String>` — удалённые ветки: локальные, влитые в HEAD, чей upstream удалён на remote; каждая удаляется `branch -d` с записью в журнал Undo; ветки HEAD и других worktree не трогаются (R-211) | M5 |
| `rebase_todo` | `repo, base` | `Vec<TodoEntry>` | M4 |
| `interactive_rebase` | `repo, base, plan: Vec<TodoEntry>` | `()` | M4 |
| `rebase_progress` | `repo` | `Option<RebaseProgress>` | M11 |
| `overlap_window` | `repo, base, window: Vec<String>` | `Vec<OverlapRow>` | M13 |
| `bypass_log` | `repo` | `Vec<Bypass>` | M10 |
| `popup_context_menu` | `items: Vec<ContextItem { id, label, enabled, separator, accelerator, children? }>, x, y`; непустой `children` делает строку подменю (`Move To ▸`), лишние разделители убираются на любой глубине | `()` | M2 |
| `open_compare_window` | `url, title` | `()` | M2 |
| `commit_template` | `repo` | `Option<String>` | M6 |
| `stage_mode` | `repo, path, executable` | `()` | M6 |
| `list_presets` | — | `Vec<PresetStatus>` | M10 |
| `install_preset` | `repo, id` | `()` | M10 |

`PresetStatus` плоский: форма TOML — дело каталога, а не webview. `toolPath` — где
инструмент нашёлся, `null` — не установлен; `installHint` тогда говорит, что делать.

`stage_mode` перерегистрирует запись индекса через `update-index --cacheinfo` с тем же
блобом: `--chmod` перечитал бы файл и затянул в индекс ещё и правки содержимого.

`popup_context_menu` — **синхронная** команда: меню на Windows показывается из главного
потока. Выбранный пункт контекстного меню возвращается тем же событием `menu-command`, что
и строка меню.

Команды окон (`open_compare_window`, `open_merge_window`, `open_blame_window`,
`close_this_window`) — наоборот, **только асинхронные**: синхронная команда выполняется
внутри обработчика WebView2, и `WebviewWindowBuilder::build()` там навсегда блокирует все
окна ([R-201](12-risks.md#r-201--дочернее-окно-чёрное-окно-и-зависшее-главное-8--в)).
Tauri сам переносит создание окна на главный поток.

`interactive_rebase` принимает `paused`: план дописывается строками `break` после каждого
применённого коммита. `overlap_window` считается только по видимому окну и фанится
`rayon` внутри `spawn_blocking` ([INV-01](01-architecture.md#inv-01)).

`TodoEntry` — `{ oid, action: pick|reword|edit|squash|fixup|drop, message }`.
Сообщение для `reword` уезжает в план строкой `exec git commit --amend`, чтобы редактор
не открывался: терминала, в котором он мог бы открыться, у приложения нет.
| `has_token` | `host` | `bool` | M1 |
| `store_token` / `forget_token` | `host[, token]` | `()` | M1 |

Токен **никогда** не возвращается наружу: `has_token` отвечает только «есть или нет»,
чтобы секрет не попадал в webview.

### Контекстные меню графа и Branches

Команды живут в `src-tauri/src/commands/ref_ops.rs`, логика — в `git_engine`
(`reset.rs`, `tags.rs`, `stash_rename.rs`, `interactive.rs`).

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `reset_to` | `repo, rev, mode: soft\|mixed\|hard\|keep\|merge` | `()` | M5 |
| `is_ancestor` | `repo, ancestor, descendant` | `bool` | M4 |
| `compare_files` | `repo, from, to` | `Vec<FileEntry>` | M5 |
| `tag_name_problem` | `repo, name` | `Option<String>` | M5 |
| `tag_message` | `repo, name` | `Option<String>` | M5 |
| `rename_tag` | `repo, from, to` | `()` | M5 |
| `rename_stash` | `repo, index, message` | `()` | M5 |
| `edit_author` | `repo, rev, name, email` | `()` | M12 |
| `push_to` | `repo, remote, refspec, onProgress: Channel<String>` | `()` | M1 |

`reset_to` с `hard` на грязном дереве сначала кладёт отслеживаемые правки в stash и
пишет его в журнал безопасности — Undo возвращает их. `tag_name_problem` зовёт
`git check-ref-format refs/tags/<имя>` один раз при подтверждении диалога, не на каждую
букву, и мимо журнала команд: отказ — это ответ, а не упавшая команда, окно ошибки Git он
не открывает. `rename_stash` сохраняет порядок списка (R-252), `edit_author` — rebase с `exec
git commit --amend --author`, как `reword`. `push_to` — один refspec: Push To, Push Up To
и push ветки или тега, которые не HEAD.

### Контекстные меню репозитория и файлов (#36, #40, #41)

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `desktop_info` | — | `DesktopInfo { fileManager, windowsShells, gitShell: string \| null, separator }` — что умеет эта платформа | M3 |
| `open_path` | `path` (абсолютный, `/`) | `()` — папка открывается сама, файл — связанной программой | M3 |
| `reveal_path` | `path` | `()` — родительская папка с выделенным элементом | M3 |
| `open_power_shell` / `open_git_shell` | `path` | `()`; только Windows, Git Bash ищется сам (R-261) | M3 |
| `move_to_trash` | `repo, paths` | `()` — в Корзину, не безвозвратно | M6 |
| `remove_from_repository` | `repo, paths, deleteLocal` | `()` — `git rm --cached` / `git rm` | M6 |
| `move_path` | `repo, from, to` | `()` — `git mv` для отслеживаемого, перенос на диске для остального; занятое имя — `InvalidState` | M6 |
| `set_index_flag` | `repo, paths, flag: "assumeUnchanged" \| "skipWorktree", on` | `()` | M6 |
| `index_editor_sides` | `repo, path` | `IndexEditorSides { head, index, worktree, binary }` | M6 |
| `write_index_editor` | `repo, path, index: string \| null, worktree: string \| null` | `()`; `null` — сторону не трогать | M6 |
| `save_blob` | `repo, rev, path, target` | `()` | M8 |
| `open_read_only` | `repo, rev, path` | `string` — путь read-only копии во временной папке | M8 |
| `apply_commit_file` | `repo, rev, path, oldPath, reverse` | `()` — `git apply --3way` изменений одного файла из коммита | M8 |
| `present_on_disk` | `repo, paths` | `string[]` — какие из путей есть в рабочей копии | M8 |

Все пути, кроме `target` и аргумента `open_path` / `reveal_path`, — относительные от корня.
`DiffSpec` получил вариант `commitVsWorkTree { oid }` — версия из коммита против файла на
диске (Compare with Working Tree).

## 5. Стриминг истории

Протокол `load_commits`:

1. UI создаёт `Channel<GraphProgress>` и один раз вызывает команду.
2. Бэкенд обходит историю отмеченных ссылок чанками по 200 коммитов, раскладывает каждый
   чанк — узел и все сегменты строки ([07-graph-rendering.md §2](07-graph-rendering.md#2-модель-данных)) —
   и дописывает его в кэш графа `AppState`. На экране один граф, поэтому и кэш один.
3. По каналу уходит только прогресс: первый чанк — сразу, дальше не чаще раза в 50 мс,
   последнее сообщение — с `isLast: true`.
4. Строки UI берёт окнами: `graph_window(repo, generation, start, count)` — срез готового
   массива в бинарной форме (base64); пустая строка, если граф этого поколения уже заменён
   или репозиторий не тот. `graph_row_of(repo, generation,
   oid)` — номер строки коммита для перехода к нему и клавиатуры, когда его блок не загружен.
5. Окно — не JSON-объекты, а колонки: числа типизированными массивами, oid — колонкой
   фиксированной ширины, остальной текст — одним UTF-8-блоком со смещениями
   (`app_state::graph_wire`, раскладка описана там же). Поверх `postMessage` байты едут
   base64-строкой; фронтенд строит представления поверх буфера и декодирует строку, только
   когда её рисуют (`$lib/graph-wire`, R-194). Формат 2: у строки — список `links` (номер
   сегмента-обрубка и oid коммита на другом конце связи, R-330).

```rust
pub struct GraphProgress {
    pub generation: u32,
    pub total: u32,      // строк разложено; список такой длины, остальное ещё обходится
    pub is_last: bool,
}
```

**Курсора нет — и не нужно.** Первая редакция плана предполагала постраничную загрузку
с `next_cursor`. Чтобы возобновить топологический обход, пришлось бы сериализовать весь
его фронт — состояние большее, чем сама страница. Обход идёт до конца одним вызовом, а
его результат остаётся в Rust: окно — срез готового массива, возобновлять нечего (R-193).

**Отмена — двухуровневая.** Когда UI бросает канал, `send` на бэкенде возвращает ошибку,
обход останавливается, и финальный чанк **не** отправляется: прерванный поток не должен
выглядеть завершённым. На фронтенде вдобавок есть счётчик `generation`: чанки потока,
запущенного для прежнего репозитория, отбрасываются, даже если успели прийти, а окно
прежнего поколения бэкенд не отдаёт.

Отфильтрованный список тоже приходит с раскладкой: линия к родителю, которого фильтр не
покажет, — сегмент с `arrow: true` (R-161).

**Почему раскладка считается на бэкенде:** алгоритм дорожек требует знания родителей за пределами
видимого окна. Считать его в JS означало бы гонять весь граф в Webview — нарушение
[INV-02](01-architecture.md#inv-02).

## 6. События

| Событие | Payload | Когда |
|---|---|---|
| `repo-changed` | `{ repo: RepoId, kind: ChangeKind }` | `fs_watcher` заметил изменение |
| `command-recorded` | `CommandNotice` | Команда git записана в журнал — для панели Output и уведомлений |
| `avatar-ready` | `{ email: String }` | Картинка автора скачана |
| `merge-resolved` | `{ repo: RepoId, path: String, … }` | Окно 3-way merge сохранило разрешение |
| `menu-command` | `String` (id команды палитры) | Выбран пункт нативного меню |
| `operation-changed` | `{ id, repo, kind, label, phase, success }` | Операция встала в очередь, началась или закончилась |
| `session-ending` | `{ reason: String }` | Windows хочет завершить сеанс, а в очереди есть операции; сеанс удержан (R-168) |

### Очередь операций

Мутации репозитория идут по одной на репозиторий, в порядке поступления. Четыре события из
постановки задачи — это одно событие `operation-changed` с полем `phase`:

| Задача | Здесь |
|---|---|
| `operation_queued` | `phase: "queued"` |
| `operation_started` | `phase: "running"` |
| `operation_finished` | `phase: "done"`, `success` — true или false |
| `operation_progress` | не в этом потоке — прогресс идёт своим `Channel` у `fetch`/`pull`/`push` |

Одно событие вместо четырёх потому, что индикатору нужен один поток сообщений с одним
`id`, а не четыре подписки, которые надо сшивать на стороне панели.

```ts
type Operation = {
  id: number,
  repo: RepoId | null,   // null — работа, не привязанная к репозиторию
  kind: OperationKind,   // "fetch" | "push" | "commit" | "merge" | …
  label: string,         // "Pushing", "Committing" — то, что показывает тулбар
  phase: "queued" | "running" | "done",
  success: boolean | null,
}
```

`list_operations` отдаёт то же самое списком: панель, открытая заново, видит очередь
целиком, а не с середины. Чтения в очередь не попадают — они идут параллельно и
отменяются через `cancel_operation`.

`ChangeKind`: `Head` · `Index` · `Refs` · `WorkingTree` · `Stash` · `Config` · `Hooks`.

Событий `repo-opened`/`repo-closed` в webview нет: `AppEvent::RepoOpened/RepoClosed` живут
только на шине `app_state`. Сырое `renderer-failed` (`renderer_failure.rs`) шлётся без
типа и страницей не слушается.
UI обновляет **только** соответствующую панель — не перезагружает всё.

## 7. Окно File Compare / 3-Way Merge

Отдельное `WebviewWindow`, создаётся через `WebviewWindowBuilder`.

- Параметры передаются через URL-запрос (`?repo=1&path=src/a.cpp&mode=merge`), а не через
  глобальное состояние — окно должно переживать перезагрузку.
- Окно вызывает те же команды, что и главное; `AppState` общий на приложение.
- Результат разрешения конфликта возвращается событием `merge-resolved`, главное окно
  обновляет список файлов.
- Закрытие окна с несохранёнными правками требует подтверждения.

Окно Investigate открывается командой `open_investigate_window(url, title)` через
`child_window::open_with_menu` внутри `blocking(...)`, с меткой `investigate-N`
(разрешения — в `capabilities/default.json`) и нативным меню File, Edit, View, Go To,
Window, Help: пункты, кроме Close, приходят в страницу событием `cogit-menu`
([R-283](12-risks.md)). Параметры — в URL (`investigate.html?repo=1&name=…&path=…&rev=…&line=…`);
из фронтенда окно открывает только `openInvestigate` (`$lib/investigate/open.ts`).

## 8. Чек-лист при добавлении команды

1. DTO объявлены с `specta::Type`, поля документированы.
2. Команда зарегистрирована в `collect_commands!`.
3. Биндинги перегенерированы, diff в `bindings.ts` осмысленный.
4. Строка добавлена в таблицу §4 этого файла.
5. Тело команды укладывается в [INV-09](01-architecture.md#inv-09) — логика в крейте, не здесь.
6. Ошибки маппятся в `CogitError`, ошибка CLI не теряет `stdout`/`stderr`.
7. Если результат может превысить 500 элементов — используется `Channel`, а не `Vec`.
8. Права в `src-tauri/capabilities/` обновлены, если команда требует новых разрешений плагина.
