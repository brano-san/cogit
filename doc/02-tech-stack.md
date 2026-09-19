# Cogit — Технологический стек

> Все версии проверены по crates.io и npm **19 сентября 2026**.
> Точные версии живут в `Cargo.toml` (`[workspace.dependencies]`) и `frontend/package.json`.
> Этот файл объясняет **зачем** каждая зависимость и **где у неё грабли**.

## 1. Тулчейн

| Инструмент | Версия | Назначение |
|---|---|---|
| Rust | **1.98.1** (stable, edition 2024) | Пин через `rust-toolchain.toml` |
| MSVC | 14.50.35717 (VS Community 2026) | Системный линковщик и CRT |
| Windows SDK | 10.0.26100 | Заголовки и библиотеки ОС |
| LLVM / lld-link | **23.1.1** | Быстрая линковка (`C:\Program Files\LLVM\bin`) |
| sccache | **0.17.0** | Кэш компиляции, `~/.cargo/bin/sccache.exe` |
| cargo-deny | **0.20.2** | Аудит лицензий и advisories |
| Node.js | 22.18.0 | Требование Vite 8: `^20.19 \|\| >=22.12` |
| npm | 10.9.3 | Менеджер пакетов фронтенда |
| WebView2 | 153.0.4234.32 | Рантайм Tauri на Windows |

Компоненты rustup: `clippy`, `rustfmt`, `rust-src`.

## 2. Ядро Rust

| Крейт | Версия | Фичи | Зачем |
|---|---|---|---|
| `tokio` | 1.53 | `rt-multi-thread`, `process`, `sync`, `fs`, `time`, `macros` | Async-рантайм, запуск `git` CLI |
| `rayon` | 1.12 | — | Параллельные CPU-задачи: дифф пачки файлов, раскладка графа |
| `crossbeam-channel` | 0.5 | — | Мост «rayon / OS-поток → async» |
| `parking_lot` | 0.12 | — | Быстрые `Mutex` / `RwLock` для синхронного состояния |
| `thiserror` | 2.0 | — | Типизированные ошибки **внутри крейтов** |
| `anyhow` | 1.0 | — | Ошибки **только** в `main.rs` и точках входа |
| `serde` | 1.0 | `derive` | Сериализация DTO |
| `serde_json` | 1.0 | — | JSON для IPC и настроек |
| `tracing` | 0.1 | — | Структурное логирование |
| `tracing-subscriber` | 0.3 | `env-filter`, `fmt`, `json` | Фильтры и форматирование |
| `tracing-appender` | 0.2 | — | Неблокирующая запись в отдельном потоке |
| `file-rotate` | 0.8 | — | Жёсткий лимит 10 МБ + 2 архива |

### Грабли: `tracing-appender` + `file-rotate`

Они **конкурируют** — у обоих есть своя ротация. Правильная схема: `file-rotate` выступает
в роли `io::Write`, а `tracing_appender::non_blocking` оборачивает его.

```rust
let file = FileRotate::new(
    log_dir.join("cogit.log"),
    AppendCount::new(2),
    ContentLimit::Bytes(10 * 1024 * 1024),
    Compression::None,
    None,
);
let (writer, guard) = tracing_appender::non_blocking(file);
```

`guard` обязан жить до конца работы приложения — при drop он сбрасывает буфер.
Потеря `guard` = молчаливая потеря последних строк лога.

## 3. Git

| Крейт | Версия | Зачем |
|---|---|---|
| `gix` | **0.87.1** | Быстрое чтение объектов, refs, index, status |
| системный `git` | 2.51 | Все мутации, сеть, хуки, rebase |

### Фичи `gix`

Дефолт (`max-performance-safe`, `comfort`, `basic`, `extras`, `auto-chain-error`, `sha1`)
**не включает** то, что нам нужно. Явно перечисляем:

```toml
gix = { version = "0.87", default-features = false, features = [
  "max-performance-safe",  # быстрые бэкенды без C-зависимостей
  "sha1",                  # ОБЯЗАТЕЛЬНА: gix-hash не собирается без sha1 или sha256
  "auto-chain-error",      # полные цепочки ошибок в Display — на этом держится INV-05
  "status",                # git status
  "blame",                 # Git Blame (M8)
  "blob-diff",             # чтение blob-ов для diff
  "dirwalk",               # обход рабочей директории
  "excludes",              # .gitignore
  "attributes",            # .gitattributes → нормализация EOL (INV-08)
  "index",                 # чтение .git/index
  "revision",              # revparse, обход истории
  "mailmap",               # .mailmap для авторов
  "parallel",              # многопоточность внутри gix
] }
```

Сетевые фичи (`blocking-network-client`, `blocking-http-transport-*`) **не подключаем**:
вся сеть идёт через системный `git`, который уже умеет SSH-агент, credential manager и прокси.
Это экономит минуты сборки и мегабайты бинарника.

### Грабли `gix`

- `gix::Repository` держит открытые файловые дескрипторы и кэши. Долго хранить в глобальном
  состоянии нельзя — изменения, сделанные CLI, могут не подхватиться. Используй `ThreadSafeRepository`
  и получай `Repository` на время операции.
- API `gix` активно меняется между минорными версиями. Перед написанием кода — **docs.rs для 0.87.1**,
  а не память модели.

## 4. Diff

| Крейт | Версия | Фичи | Зачем |
|---|---|---|---|
| `imara-diff` | **0.2.0** | `unified_diff` (дефолт) | Блочный diff, алгоритм Histogram |
| `similar` | **3.2.0** | `text`, **`inline`**, `unicode` | Внутристрочный word/char diff |
| `tree-sitter` | 0.27 | — | AST-диффы и синтаксическое 3-way слияние |

### Грабли: `imara-diff` 0.2 ≠ 0.1

API переписан полностью. Все примеры в интернете и в обучающих данных моделей — от 0.1.x.
**Обязательно** открыть `docs.rs/imara-diff/0.2.0` перед первой строкой кода `diff_engine`.

### Грабли: `similar` без `inline`

Фича `inline` **не входит в дефолт**. Без неё нет `TextDiff::iter_inline_changes`,
то есть нет подсветки изменённых слов внутри строки — а это половина ценности diff-вьюера.

### Почему два движка diff

`imara-diff` быстрее на больших файлах и даёт Histogram (лучшая читаемость блоков),
но не умеет внутристрочное сравнение. `similar` умеет inline, но медленнее на больших входах.
Схема: блоки считает `imara-diff`, затем только для изменённых пар строк вызывается `similar`.

## 5. Файловый мониторинг

| Крейт | Версия | Фичи | Зачем |
|---|---|---|---|
| `notify` | 8.2 | — | Кроссплатформенные события ФС |
| `notify-debouncer-mini` | 0.7 | `crossbeam-channel` | Дебаунс 100 мс |
| `ignore` | 0.4 | — | Фильтрация путей по `.gitignore` |

Ограничения области наблюдения — [INV-06](01-architecture.md#inv-06).

## 6. Система и безопасность

| Крейт | Версия | Зачем |
|---|---|---|
| `keyring` | **4.2.0** | Токены в хранилище ОС |
| `directories` | 6.0 | Пути конфигов и кэша по конвенциям ОС |
| `tauri-plugin-opener` | 2.5 | Открыть путь в проводнике / URL в браузере |
| `tauri-plugin-shell` | 2.3 | Запуск внешнего терминала |
| `tauri-plugin-store` | 2.4 | Персистентные настройки UI |
| `tauri-plugin-window-state` | 2.4 | Сохранение геометрии окон |
| `tauri-plugin-dialog` | 2.7 | Системные диалоги выбора папки |
| `tauri-plugin-clipboard-manager` | 2.3 | Кнопка «Copy Output» в диалоге ошибок |

### Грабли: `keyring` 4.x

В 4-й мажорной версии набор фич перестроен: остались только `cli`, `default`, `v1`
(`default = ["v1"]`), а прежние `windows-native` / `apple-native` / `sync-secret-service` исчезли —
бэкенды подключаются внутри крейта по целевой платформе.
**Задача M1:** проверить на docs.rs, как в 4.x создаётся `Entry` и что происходит на Linux
без Secret Service (нужен fallback в память, как требует спека).

### Аутентификация в сети

Cogit **не реализует** свою аутентификацию. Всё делегируется системному `git`:
SSH-агент / OpenSSH, `git-credential-manager`, `~/.gitconfig`.
`keyring` используется только для токенов, которые вводит сам пользователь в настройках Cogit.

## 7. Tauri

| Пакет | Версия | Зачем |
|---|---|---|
| `tauri` (Rust) | **2.11.5** | Рантайм |
| `tauri-build` | 2.x | Сборка |
| `@tauri-apps/api` | 2.11.1 | JS-клиент |
| `@tauri-apps/cli` | 2.11.4 | `tauri dev` / `tauri build` |
| `tauri-specta` | **=2.0.0-rc.25** | Генерация типизированных биндингов |
| `specta` / `specta-typescript` | **=2.0.0-rc.25** / 0.0.12 | Экспорт типов Rust → TS |

### Контекстные меню — без плагина

`tauri-plugin-context-menu` (0.8.2, последнее обновление **октябрь 2024**) написан под Tauri v1
и с v2 несовместим. Используем **встроенный** `tauri::menu`:

```rust
use tauri::menu::{ContextMenu, Menu, MenuItem};
let menu = Menu::with_items(app, &[&MenuItem::with_id(app, "stage", "Stage", true, None::<&str>)?])?;
menu.popup(window)?;
```

Это нативные меню ОС — ровно то, что требовала спека, без стороннего кода.
Подробности решения — [12-risks.md](12-risks.md).

## 8. Фронтенд

| Пакет | Версия | Зачем |
|---|---|---|
| `svelte` | **5.57.1** | UI-фреймворк, руны |
| `vite` | **8.3.0** | Сборка и dev-сервер |
| `@sveltejs/vite-plugin-svelte` | 7.3.0 | Интеграция (peer: `vite ^8`, `svelte ^5.46.4`) |
| `typescript` | **6.0.3** | Типы — **не 7.x**, см. ниже |
| `svelte-check` | 4.7.6 | Проверка типов в `.svelte` |
| `vitest` | 5.0.1 | Юнит-тесты |

### Грабли: TypeScript 7 ломает svelte-check

Актуальный TS — **7.0.2** (нативный порт на Go), но `svelte-check@4.7.6` объявляет
`peerDependencies: { typescript: "^5.0.0 || ^6.0.0" }`. Поэтому пин **`typescript@~6.0.3`**.
Как только `svelte-check` объявит поддержку TS 7 — обновляемся; триггер записан в [12-risks.md](12-risks.md).

Svelte 5 **без SvelteKit**: приложение — SPA в Webview, SSR и роутинг не нужны,
а SvelteKit добавил бы адаптеры и слой, который нечем оправдать.

## 9. CodeMirror 6

| Пакет | Версия |
|---|---|
| `codemirror` | 6.0.2 |
| `@codemirror/state` / `view` | 6.7.5 / 6.43.12 |
| `@codemirror/language` | 6.12.4 |
| `@codemirror/commands` / `search` | 6.11.1 / 6.7.2 |
| `@codemirror/merge` | 6.12.2 |
| `@codemirror/lang-cpp` | 6.0.3 |
| `@codemirror/lang-markdown` | 6.5.2 |
| `@codemirror/lang-javascript` | 6.2.5 |
| `@codemirror/lang-html` / `lang-css` | 6.4.12 / 6.3.1 |

Пакеты `@codemirror/lang-*` подтягивают соответствующие `@lezer/*` сами — отдельно `@lezer/cpp`
ставить не нужно. Прямая зависимость на `@lezer/*` нужна только при написании своей грамматики.

`@codemirror/merge` даёт готовый unified/split-вид. **Решение по M7:** оценить его до того,
как писать свой side-by-side. Если он покрывает синхроскролл и сворачивание блоков —
пишем только слой сплайнов поверх; если нет — свой вьюер на двух `EditorView`.

## 10. Тестирование

| Крейт / пакет | Версия | Зачем |
|---|---|---|
| `insta` | 1.48 | Snapshot-тесты раскладки графа и дифов |
| `tempfile` | 3.27 | Временные репозитории в фикстурах |
| `vitest` | 5.0.1 | Тесты логики фронтенда |

Стратегия — [09-testing.md](09-testing.md).

## 11. Политика обновления зависимостей

1. **Единый источник версий:** только `[workspace.dependencies]` ([INV-11](01-architecture.md#inv-11)).
2. **Перед добавлением зависимости** — `cargo deny check`. Запрещены: GPL/AGPL, крейты
   с открытыми advisories, крейты без обновлений более 18 месяцев без явного обоснования в этом файле.
3. **Dependabot** открывает PR еженедельно; мажорные обновления `gix`, `tauri`, `svelte`
   рассматриваются вручную — у них ломающие изменения API.
4. **Никогда не доверяй памяти модели о версии API.** Крейты `gix`, `imara-diff`, `keyring`, `similar`
   пережили ломающие мажоры в 2025–2026. Открывай docs.rs для **точной** версии из `Cargo.lock`.
