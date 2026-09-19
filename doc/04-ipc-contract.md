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
| Ошибки | Все команды возвращают `Result<T, CogitError>`; `CogitError` — размеченное объединение |
| Отсутствие значения | `Option<T>` → `T \| null`, никаких «пустых строк вместо null» |

### Тип ошибки

```rust
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(tag = "kind", content = "data")]
pub enum CogitError {
    #[error(transparent)]
    GitCommand(GitCommandError),          // ошибка CLI, показывается в Git Error Dialog
    #[error("repository not found: {0}")]
    RepoNotFound(String),
    #[error("repository is busy: {0}")]
    RepoBusy(String),                     // index.lock и подобное
    #[error("invalid state: {0}")]
    InvalidState(String),                 // detached HEAD там, где нужна ветка
    #[error("io error: {0}")]
    Io(String),
    #[error("internal error: {0}")]
    Internal(String),
}
```

Размеченное объединение, а не строка: фронтенду нужно **различать** ошибку CLI (открыть
специальный диалог с сырым выводом) и остальные (показать тост).

## 4. Команды

Ниже — контракт. Реализуются по модулям; колонка «Модуль» указывает, когда команда появляется.

### Репозитории

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `open_repository` | `path: String` | `RepoSummary` | M1 |
| `close_repository` | `repo: RepoId` | `()` | M1 |
| `list_repositories` | — | `Vec<RepoEntry>` | M3 |
| `repo_state` | `repo: RepoId` | `RepoState` | M1 |
| `list_submodules` | `repo: RepoId` | `Vec<Submodule>` | M3 |
| `list_worktrees` | `repo: RepoId` | `Vec<Worktree>` | M3 |

### История и граф

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `load_commits` | `repo, channel: Channel<GraphChunk>` | `()` | M4 |
| `commit_details` | `repo, oid` | `CommitDetails` | M4 |
| `commit_files` | `repo, oid` | `Vec<FileEntry>` | M6 |
| `search_commits` | `repo, query: CommitQuery, channel` | `()` | M4 |

### Ссылки

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `list_refs` | `repo` | `RefTree` | M5 |
| `list_stashes` | `repo` | `Vec<StashEntry>` | M5 |
| `stash_contents` | `repo, index: usize` | `Vec<FileEntry>` | M5 |
| `list_reflog` | `repo` | `Vec<ReflogEntry>` | M5 |

### Diff

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `diff_file` | `repo, spec: DiffSpec` | `FileDiff` | M7 |
| `diff_working_tree` | `repo, path` | `FileDiff` | M7 |
| `merge_conflict` | `repo, path` | `ThreeWayDiff` | M7 |

`DiffSpec` описывает, что с чем сравнивается: `WorkTreeVsIndex`, `IndexVsHead`,
`CommitVsParent { oid }`, `CommitVsCommit { a, b }`, `StashVsParent { index }`.

### Мутации

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `stage_paths` / `unstage_paths` | `repo, paths: Vec<String>` | `()` | M6 |
| `stage_hunk` | `repo, patch: String` | `()` | M6 |
| `discard_paths` | `repo, paths` | `()` | M6 |
| `commit` | `repo, message, amend: bool, no_verify: bool` | `String` (oid) | M6 |
| `checkout` | `repo, target: CheckoutTarget` | `()` | M5 |
| `create_branch` / `delete_branch` | `repo, ...` | `()` | M5 |
| `merge` / `rebase` / `cherry_pick` / `revert` | `repo, ...` | `()` | M5 |
| `stash_push` / `apply` / `pop` / `drop` | `repo, ...` | `()` | M5 |
| `fetch` / `pull` / `push` | `repo, remote, refspec, channel: Channel<Progress>` | `()` | M1 |
| `undo_last` | `repo` | `UndoResult` | M5 |

Все мутации возвращают `Result<_, CogitError>` и при неуспехе CLI — вариант `GitCommand`.

### Служебные

| Команда | Вход | Выход | Модуль |
|---|---|---|---|
| `app_info` | — | `AppInfo { version, git_version, log_path }` | M0 |
| `command_log` | `limit` | `Vec<CommandLogEntry>` | M2 |
| `open_in_explorer` / `open_in_terminal` | `path` | `()` | M3 |

## 5. Стриминг истории

Протокол `load_commits`:

1. UI создаёт `Channel<GraphChunk>` и один раз вызывает команду.
2. Бэкенд обходит историю от всех типов веток и шлёт чанки по 200 коммитов; каждый чанк
   уже содержит готовую раскладку дорожек для своих строк.
3. Обход завершается пустым чанком с `isLast: true`.

```rust
#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GraphChunk {
    pub commits: Vec<CommitRow>,
    pub lanes: Vec<LaneAssignment>,
    pub edges: Vec<GraphEdge>,
    pub max_lane: u16,
    pub is_last: bool,
}
```

**Курсора нет — и не нужно.** Первая редакция плана предполагала постраничную загрузку
с `next_cursor`. Чтобы возобновить топологический обход, пришлось бы сериализовать весь
его фронт — состояние большее, чем сама страница. Один вызов, который стримит всё
и прерывается по требованию, проще и дешевле.

**Отмена — двухуровневая.** Когда UI бросает канал, `send` на бэкенде возвращает ошибку,
обход останавливается, и финальный чанк **не** отправляется: прерванный поток не должен
выглядеть завершённым. На фронтенде вдобавок есть счётчик `generation`: чанки потока,
запущенного для прежнего репозитория, отбрасываются, даже если успели прийти.

**Почему раскладка считается на бэкенде:** алгоритм дорожек требует знания родителей за пределами
видимого окна. Считать его в JS означало бы гонять весь граф в Webview — нарушение
[INV-02](01-architecture.md#inv-02).

## 6. События

| Событие | Payload | Когда |
|---|---|---|
| `repo-changed` | `{ repo: RepoId, kind: ChangeKind }` | `fs_watcher` заметил изменение |
| `repo-opened` / `repo-closed` | `{ repo: RepoId }` | Изменился состав открытых репозиториев |
| `operation-started` / `operation-finished` | `{ id, label, result }` | Для спиннера в тулбаре |
| `git-command-logged` | `CommandLogEntry` | Для панели Output |

`ChangeKind`: `Head` · `Index` · `Refs` · `WorkingTree` · `Stash` · `Config`.
UI обновляет **только** соответствующую панель — не перезагружает всё.

## 7. Окно File Compare / 3-Way Merge

Отдельное `WebviewWindow`, создаётся через `WebviewWindowBuilder`.

- Параметры передаются через URL-запрос (`?repo=1&path=src/a.cpp&mode=merge`), а не через
  глобальное состояние — окно должно переживать перезагрузку.
- Окно вызывает те же команды, что и главное; `AppState` общий на приложение.
- Результат разрешения конфликта возвращается событием `merge-resolved`, главное окно
  обновляет список файлов.
- Закрытие окна с несохранёнными правками требует подтверждения.

## 8. Чек-лист при добавлении команды

1. DTO объявлены с `specta::Type`, поля документированы.
2. Команда зарегистрирована в `collect_commands!`.
3. Биндинги перегенерированы, diff в `bindings.ts` осмысленный.
4. Строка добавлена в таблицу §4 этого файла.
5. Тело команды укладывается в [INV-09](01-architecture.md#inv-09) — логика в крейте, не здесь.
6. Ошибки маппятся в `CogitError`, ошибка CLI не теряет `stdout`/`stderr`.
7. Если результат может превысить 500 элементов — используется `Channel`, а не `Vec`.
8. Права в `src-tauri/capabilities/` обновлены, если команда требует новых разрешений плагина.
