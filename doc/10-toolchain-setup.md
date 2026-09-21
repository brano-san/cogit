# Cogit — Установка окружения

> Состояние на **19 сентября 2026**. Всё перечисленное уже установлено на машине разработчика —
> этот файл нужен для воспроизведения на новой машине и для диагностики.

## 1. Что требуется

| Компонент | Версия | Обязателен |
|---|---|---|
| Rust (rustup, stable) | 1.98.1 | да |
| MSVC + Windows SDK | VS 2026 / 10.0.26100 | да (Windows) |
| WebView2 Runtime | 153+ | да (Windows) |
| Node.js | 22.18+ | да |
| Git | 2.51+ | да |
| LLVM (`lld-link`) | 23.1.1 | да — настроен как линковщик |
| sccache | 0.17.0 | да — прописан в `.cargo/config.toml` |
| cargo-deny | 0.20.2 | для аудита зависимостей |

WebView2 предустановлен в Windows 11. На Windows 10 может потребоваться Evergreen Runtime.

## 2. Установка с нуля (Windows)

```powershell
# 1. Rust
winget install --id Rustlang.Rustup -e --source winget --accept-source-agreements --accept-package-agreements
rustup default stable
rustup component add clippy rustfmt rust-src

# 2. LLVM (даёт lld-link)
winget install --id LLVM.LLVM -e --source winget --accept-package-agreements

# 3. sccache
winget install --id Mozilla.sccache -e --source winget --accept-package-agreements

# 4. cargo-deny (сборка из исходников, ~2 минуты)
cargo install cargo-deny --locked

# 5. Node.js (если ещё нет)
winget install --id OpenJS.NodeJS.LTS -e --source winget --accept-package-agreements
```

Инструменты C++ ставятся вместе с Visual Studio: рабочая нагрузка
**Desktop development with C++** (компонент `Microsoft.VisualStudio.Component.VC.Tools.x86.x64`).
Без неё Rust не сможет слинковать бинарник.

### Доводка PATH

`winget` **не добавляет** LLVM в `PATH`, а `sccache` кладёт в свой каталог пакетов.
Оба шага нужны вручную:

```powershell
# LLVM в пользовательский PATH
$llvm = "C:\Program Files\LLVM\bin"
$u = [Environment]::GetEnvironmentVariable('PATH','User')
if ($u -notlike "*$llvm*") { [Environment]::SetEnvironmentVariable('PATH', "$($u.TrimEnd(';'));$llvm", 'User') }

# sccache в ~/.cargo/bin (он уже в PATH)
$pkg = Get-ChildItem "$env:LOCALAPPDATA\Microsoft\WinGet\Packages" -Filter sccache.exe -Recurse | Select-Object -First 1
Copy-Item $pkg.FullName "$env:USERPROFILE\.cargo\bin\sccache.exe" -Force
```

Новый `PATH` виден только в **новых** терминалах — текущий перезапустить.

## 3. Проверка

```powershell
rustc --version        # rustc 1.98.1 (48a229cea 2026-09-01)
cargo --version        # cargo 1.98.1
lld-link --version     # LLD 23.1.1
sccache --version      # sccache 0.17.0
cargo deny --version   # cargo-deny 0.20.2
node --version         # v22.18.0
git --version          # git version 2.51.0.windows.1
```

Наличие MSVC:

```powershell
& "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe" `
  -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
  -format value -property installationPath
```

Пустой вывод означает, что рабочая нагрузка C++ не установлена.

## 4. Сборка проекта

```powershell
# Зависимости фронтенда
cd frontend; npm ci; cd ..

# Проверка Rust без полной сборки — самый быстрый цикл
cargo check --workspace

# Запуск в режиме разработки (открывает окно с hot-reload фронтенда)
cd frontend; npm run tauri dev

# Релизная сборка с установщиком
cd frontend; npm run tauri build
```

Первая сборка занимает **10–25 минут**: `gix` и Tauri тянут несколько сотен крейтов.
Последующие — секунды благодаря `sccache` и инкрементальной компиляции.

## 5. Ускорение сборки

`.cargo/config.toml` в корне уже настроен:

| Настройка | Эффект |
|---|---|
| `rustc-wrapper = "sccache"` | Кэш результатов компиляции между сборками и ветками |
| `linker = "lld-link.exe"` | Линковка в несколько раз быстрее штатного `link.exe` |
| `[profile.dev.package."*"] opt-level = 2` | Зависимости оптимизированы, свой код — нет. `gix` и diff в отладке иначе работают недопустимо медленно |
| `debug = "line-tables-only"` для dev | Короче время линковки, стектрейсы сохраняются |

Статистика кэша:

```powershell
sccache --show-stats
```

Низкий процент попаданий после первой сборки — повод проверить, что `RUSTC_WRAPPER`
не переопределён переменной окружения.

### Опыты с профилем — в отдельный `target`

Cargo **никогда не удаляет старое**. Любая правка `[profile.release]` меняет хеш и даёт
полный новый комплект артефактов для всего графа зависимостей; прежний остаётся лежать
навсегда. То же делает сборка с другим набором фич.

21 сентября разбор падения на старте потребовал четырёх правок профиля (`strip`, `debug`,
`lto`, `debug-assertions`) и сборок с `--features tauri/custom-protocol`. `target/` вырос
до 55 ГБ: 25 ГБ устаревших вариантов в `debug/deps` (двадцать пять копий одного `libgix`)
и 21 ГБ инкрементального кэша.

Поэтому временные опыты гоняем с отдельным каталогом, который потом просто удаляется:

```powershell
$env:CARGO_TARGET_DIR = "$PWD\target-probe"
cargo build --release -p cogit --bin cogit --features tauri/custom-protocol
Remove-Item -Recurse -Force .\target-probe
```

Вернуть место потом: `target/debug` удаляется целиком, это только вывод компилятора, а
`sccache` делает пересборку дешевле, чем кажется. Установщики лежат в
`target/release/bundle` — их `cargo clean` унесёт с собой, так что сперва скопировать.

## 6. Настройка репозитория

```powershell
git config core.hooksPath .githooks    # включить pre-commit хук
```

Это выполняется **один раз на клон**: Git не позволяет коммитить содержимое `.git/hooks`,
поэтому хуки лежат в `.githooks/` и подключаются через конфиг.

## 7. Частые проблемы

| Симптом | Причина | Решение |
|---|---|---|
| `link.exe not found` | Нет рабочей нагрузки C++ | Установить Desktop development with C++ |
| `lld-link: command not found` | LLVM не в `PATH` | См. §2, перезапустить терминал |
| `sccache: server startup failed` | Порт 4226 занят | `sccache --stop-server`, затем `--start-server` |
| Сборка игнорирует sccache | `RUSTC_WRAPPER` переопределён | Проверить переменные окружения |
| `tauri dev` открывает белое окно | Vite ещё не поднялся или ошибка сборки фронтенда | Смотреть вывод терминала, не окно |
| `WebView2 not found` | Нет рантайма (Windows 10) | Установить Evergreen Runtime |
| Ошибки `gix` о версии API | Код писался по памяти о старой версии | Открыть docs.rs для версии из `Cargo.lock` |
| `error: linking with lld-link failed` | Несовместимость флагов lld и MSVC | Временно убрать `linker` из `.cargo/config.toml`, собрать на `link.exe`, завести запись в `12-risks.md` |
| Долгая первая сборка | Так и должно быть | Дождаться; последующие быстрые |
| `cargo: command not found` при коммите из SmartGit, Fork, IDE | GUI-клиент запущен раньше, чем ставился тулчейн, и держит старый `PATH` | Хук сам добавляет `~/.cargo/bin` и `LLVM/bin`, так что это уже починено. Если сообщение всё же появилось — тулчейна действительно нет, ставить по §2 |

## 8. Линукс и macOS

Поддержка заявлена как «совместимость»: код пишется кроссплатформенным, регулярная сборка
и проверка — только на Windows.

Что потребуется на Linux:

```bash
# Debian/Ubuntu — системные зависимости Tauri v2
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev

# Линковщик mold вместо lld
sudo apt install mold clang
```

Секция для Linux в `.cargo/config.toml` уже присутствует, но **не проверена сборкой** —
при первом запуске на Linux её нужно валидировать и зафиксировать результат в этом файле.

Отдельного внимания на Linux требует `keyring`: без работающего Secret Service нужен
fallback в память, как требует исходная спецификация. Задача относится к M1.
