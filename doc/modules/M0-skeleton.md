# M0 · Костяк проекта и тулчейн

**Статус:** `DONE` (19 сентября 2026, кроме одной ручной проверки в T0.6) · **Веха:** A · **Зависит от:** — · **Блокирует:** всё

Цель: собрать пустой, но **работающий** каркас, в который дальше только добавляются модули.
Костяк без проверенной сборки бесполезен — он создаёт иллюзию готовности.

## Задачи

### T0.1 · Тулчейн
- [x] rustup + stable 1.98.1, компоненты `clippy`, `rustfmt`, `rust-src`
- [x] LLVM 23.1.1 (`lld-link`), путь добавлен в пользовательский `PATH`
- [x] sccache 0.17.0 в `~/.cargo/bin`
- [x] cargo-deny 0.20.2
- [x] Проверено наличие MSVC 14.50, Windows SDK 10.0.26100, WebView2, Node 22.18, Git 2.51

**DoD:** все команды из [10-toolchain-setup.md §3](../10-toolchain-setup.md#3-проверка) отвечают ожидаемыми версиями.

### T0.2 · Файлы корня репозитория
- [x] `Cargo.toml` — workspace, `[workspace.dependencies]` со всеми версиями ([INV-11](../01-architecture.md#inv-11))
- [x] `rust-toolchain.toml` — пин 1.98.1 и компоненты
- [x] `.cargo/config.toml` — sccache, lld-link, профили
- [x] `rustfmt.toml`, `[workspace.lints]` в `Cargo.toml`
- [x] `.gitignore`, `.claudeignore`, `.editorconfig`
- [x] `deny.toml` — политика лицензий и advisories
- [x] `.github/dependabot.yml` — cargo и npm, еженедельно
- [x] `.githooks/pre-commit` + инструкция `git config core.hooksPath .githooks`
- [x] `CLAUDE.md` — конституция проекта (на английском)

**DoD:** `cargo metadata` отрабатывает без ошибок; `cargo deny check` проходит.

### T0.3 · Крейты-заглушки
- [x] `crates/git_engine`, `diff_engine`, `graph_engine`, `fs_watcher`, `app_state`, `test_fixtures`
- [x] У каждого — `Cargo.toml` с реальными зависимостями и `lib.rs` с модульной структурой
- [x] Каждый крейт компилируется и содержит хотя бы один проходящий тест

**DoD:** `cargo test --workspace` зелёный; `cargo clippy --workspace --all-targets -- -D warnings` чисто.

### T0.4 · Слой Tauri
- [x] `src-tauri/Cargo.toml`, `tauri.conf.json`, `build.rs`
- [x] `src/lib.rs` — сборка приложения, плагины, `.manage(AppState)`
- [x] `src/main.rs` — точка входа
- [x] `src/commands/mod.rs` — команда `app_info`
- [x] `capabilities/default.json` — минимальный набор разрешений
- [x] Генерация `bindings.ts` через `tauri-specta`
- [x] Инициализация `tracing` с ротацией 10 МБ ([INV-04](../01-architecture.md#inv-04))

**DoD:** `cargo build -p cogit` собирается; при запуске создаётся файл лога в `app_log_dir`.

### T0.5 · Фронтенд
- [x] `package.json` с версиями из [02-tech-stack.md](../02-tech-stack.md) (TypeScript `~6.0.3`)
- [x] `vite.config.ts`, `tsconfig.json`, `svelte.config.js`
- [x] `index.html`, `src/main.ts`, `src/App.svelte`
- [x] `src/app.css` — все токены из [06-design-system.md](../06-design-system.md)
- [x] Компонент сплиттера и раскладка из 5 панелей с заголовками
- [x] Тулбар (40 px) и статус-бар (24 px) с заглушками
- [x] `src/lib/ipc/` — обёртка над сгенерированными биндингами

**DoD:** `npm run check` без ошибок; `npm run build` собирается.

### T0.6 · Сквозная проверка
- [x] `npm run tauri dev` открывает тёмное окно с работающими сплиттерами
- [x] Версия приложения в статус-баре получена **реальным** вызовом `app_info` через биндинги
- [ ] Перетаскивание сплиттеров и сохранение размеров между запусками — **проверить вручную**
      (реализовано и покрыто юнит-тестами `clampFraction`, но перетаскивание мышью и
      восстановление после перезапуска глазами не проверялись)

**DoD вехи A (часть M0):** окно открывается, раскладка работает, IPC жив, логи пишутся.

## Проверочные команды

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
npm run check
npm run test
npm run tauri dev
```

## Риски модуля

| Риск | Проявление | Действие |
|---|---|---|
| `lld-link` несовместим с флагами rustc | Ошибка линковки | Убрать `linker` из `.cargo/config.toml`, записать в [12-risks.md](../12-risks.md) (D-02, план Б) |
| `tauri-specta` не собирается с Tauri 2.11 | Ошибка сборки `src-tauri` | Перейти на `ts-rs`, см. D-07 |
| Первая сборка кажется зависшей | 10–25 минут тишины | Это нормально; следить за выводом cargo |

## Что НЕ делается в M0

Никакой логики Git. Панели пустые. Задача модуля — доказать, что цепочка
`Rust → specta → TypeScript → Svelte → окно` собрана и работает.
Соблазн «заодно прикрутить чтение веток» относится к Вехе B.
