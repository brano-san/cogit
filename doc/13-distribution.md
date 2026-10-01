# Cogit — Распространение и запуск у пользователя

> Что именно получает конечный пользователь и что должно быть у него на машине.
> Проверено на релизной сборке 19 сентября 2026.
> Выпуск через GitHub Actions — [RELEASING.md](RELEASING.md).

## 1. Что производит `npm run tauri build`

| Артефакт | Путь | Назначение |
|---|---|---|
| `cogit.exe` | `target/release/` | Самодостаточный бинарник, portable-вариант |
| `Cogit_0.1.0_x64_en-US.msi` | `target/release/bundle/msi/` | Установщик WiX для корпоративного развёртывания |
| `Cogit_0.1.0_x64-setup.exe` | `target/release/bundle/nsis/` | Установщик NSIS, обычный пользовательский вариант |

`cogit.pdb` рядом с exe — отладочные символы. Пользователю не нужен, но его стоит
сохранять для каждого релиза: без него стектрейсы из отчётов об ошибках нечитаемы.

## 2. Что вшито внутрь exe

- **Весь фронтенд.** Макрос `tauri::generate_context!` запекает содержимое `frontend/dist`
  (HTML, CSS, JS) в бинарник, сжатым brotli. Папка с ассетами рядом не нужна.
- **Манифест ComCtl32 v6.** Встраивается `tauri-build`. Именно поэтому бинарник запускается,
  а тест-харнесс без манифеста — нет ([D-10](12-risks.md)).
- **WebView2Loader.** Слинкован статически, отдельной DLL рядом не требуется.
- **Иконки и метаданные версии** — через ресурсы Windows.
- **Список сторонних лицензий.** `build.rs` вызывает `cargo metadata --offline --locked`
  и вшивает список крейтов, которые линкуются в `cogit.exe` (имя, версия, SPDX-лицензия,
  репозиторий); сборка Vite кладёт рядом со страницей `third-party-licences.txt` со
  списком npm-пакетов, попавших в бандл. About ▸ Third-party licenses склеивает оба и
  открывает текстовым файлом. Полных текстов лицензий нет: для этого нужен `cargo-about`,
  на машине сборки его нет ([R-173](12-risks.md)). Если `cargo metadata` не ответил,
  сборка не падает — в списке будет строка о причине.

Проверено: `cogit.exe`, скопированный **в одиночестве** в пустую папку, запускается
и полностью отрисовывает интерфейс.

## 3. Что нужно на машине пользователя

### WebView2 Runtime — обязательно

Tauri не тащит с собой браузерный движок, он использует системный Edge WebView2.

| Система | Наличие |
|---|---|
| Windows 11 | Предустановлен всегда |
| Windows 10 | Практически всегда (приезжает с обновлениями Edge), но гарантии нет |
| Windows Server / LTSC | Часто отсутствует |

Установщики закрывают этот пробел: режим по умолчанию `downloadBootstrapper` докачивает
рантайм при установке, если его нет. **Голый `cogit.exe` этого не делает** — на машине
без WebView2 он просто не откроет окно.

Если понадобится полностью автономный установщик (без обращения к сети), в
`tauri.conf.json` есть режим `embedBootstrapper` или `offlineInstaller` — ценой
примерно +130 МБ к размеру.

### Visual C++ Redistributable — **не требуется**

Это стоит зафиксировать, потому что интуиция подсказывает обратное. Таблица импорта
релизного `cogit.exe`:

```
ADVAPI32  api-ms-win-core-synch-l1-2-0  api-ms-win-crt-convert-l1-1-0
api-ms-win-crt-heap-l1-1-0  api-ms-win-crt-locale-l1-1-0  api-ms-win-crt-math-l1-1-0
api-ms-win-crt-runtime-l1-1-0  api-ms-win-crt-stdio-l1-1-0  api-ms-win-crt-string-l1-1-0
bcryptprimitives  comctl32  dwmapi  gdi32  kernel32  ntdll  ole32  oleaut32
shell32  shlwapi  user32
```

`VCRUNTIME140.dll` в списке **отсутствует**; таблица отложенных импортов пуста.
Все `api-ms-win-crt-*` — это UCRT, часть самой Windows 10/11.

Отсюда: добавлять `-C target-feature=+crt-static` не нужно. Отладочная сборка
`VCRUNTIME140.dll` импортирует, релизная — нет, поэтому проверять зависимости
надо именно на релизном бинарнике.

## 4. Как проверять перед релизом

```powershell
# 1. Полная сборка, включая упаковку — «собралось» и «упаковалось» не одно и то же (R-19)
npm run tauri build

# 2. Фактические зависимости релизного бинарника
llvm-readobj --coff-imports target\release\cogit.exe

# 3. Запуск в изоляции: только exe, пустая папка
mkdir $env:TEMP\cogit-isolated
copy target\release\cogit.exe $env:TEMP\cogit-isolated
& $env:TEMP\cogit-isolated\cogit.exe
```

Третий шаг ловит забытые внешние файлы: если приложение работает из `target/release`,
но не из пустой папки — значит, что-то не вшито.

## 5. Подпись кода

Обвязка готова, сертификата нет. Он не хранится в репозитории и не одинаков на разных
машинах, поэтому подпись включается наложением конфигурации:

```powershell
Copy-Item src-tauri\tauri.signing.conf.example.json src-tauri\tauri.signing.conf.json
# вписать отпечаток своего сертификата, затем:
npm run tauri build -- --config src-tauri/tauri.signing.conf.json
```

`tauri.signing.conf.json` в `.gitignore`; рядом лежит отслеживаемый пример. Отпечаток —
SHA-1 сертификата из личного хранилища: `Get-ChildItem Cert:\CurrentUser\My`.

- [x] Наложение принимается сборкой — проверено на примере
- [ ] **Подписанного артефакта нет: сертификата не существует.** Пока его нет,
      SmartScreen предупреждает при первом запуске скачанного установщика

**Почему отпечаток, а не `signCommand`:** для облачного подписанта (Azure Trusted Signing
и подобных) в том же файле есть `signCommand`. Для обычного сертификата в хранилище
Windows отпечатка достаточно, и он не тащит за собой ещё один инструмент.

## 6. Автообновление

Подключены `tauri-plugin-updater` 2.12 и `tauri-plugin-process` (перезапуск после
установки).

**Проверка вручную** — Help ▸ Check for Updates…, она же команда `check-updates` в палитре.
Сообщает и когда обновлений нет, и когда проверка не удалась.

**Проверка при старте** — галочка «Check for updates when Cogit starts» в Settings ▸
Advanced ▸ Updates. По умолчанию **снята**, как и у аватаров: в сеть не ходим, пока не
разрешили. Когда стоит — проверка молчит, если ставить нечего или сеть недоступна, и
спрашивает, только когда обновление действительно есть.

| Что | Где |
|---|---|
| Публичный ключ, конечная точка, режим установки | `src-tauri/tauri.conf.json`, `plugins.updater` |
| Приватный ключ | `%USERPROFILE%\.cogit\cogit-updater.key` — **вне репозитория** |
| Решение «нашлось / согласились / поставилось» | [lib/updates.ts](../frontend/src/lib/updates.ts), 15 тестов |

Конечная точка — `releases/latest/download/latest.json` в репозитории на GitHub.
`createUpdaterArtifacts: true` кладёт рядом с установщиками `.sig`; `latest.json`
собирается при выпуске релиза из них.

Сборка с подписью обновления. Переменная берёт **содержимое** ключа, а не путь:
`TAURI_SIGNING_PRIVATE_KEY_PATH` бандлер игнорирует и сообщает «a public key has been
found, but no private key».

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.cogit\cogit-updater.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
npm run tauri build
```

**Приватный ключ нужно сохранить в надёжном месте.** Он — единственное доказательство, что
обновление выпустили вы. Потеряв его, обновить уже установленные копии будет нечем: они
откажутся принимать пакет, подписанный другим ключом.

- [x] Плагины подключены, разрешения выданы, пункт меню и галочка на месте
- [x] Подпись работает: сборка выпускает `.sig` рядом с обоими установщиками
- [x] Ветки решения покрыты тестами, включая отказ пользователя и обрыв установки
- [x] Старт не пострадал: десять запусков подряд, ни одного падения
- [ ] **Сквозная проверка требует выпущенного релиза:** пока в `releases/latest` нет
      `latest.json`, проверка честно отвечает, что обновлений нет

## 7. Проверка на чистой Windows

Таблица импорта пересобрана на текущем релизном бинарнике
(`llvm-readobj --coff-imports target\release\cogit.exe`):

```
advapi32  api-ms-win-core-synch-l1-2-0  api-ms-win-crt-convert-l1-1-0
api-ms-win-crt-heap-l1-1-0  api-ms-win-crt-locale-l1-1-0  api-ms-win-crt-math-l1-1-0
api-ms-win-crt-runtime-l1-1-0  api-ms-win-crt-stdio-l1-1-0  api-ms-win-crt-string-l1-1-0
bcrypt  bcryptprimitives  combase  comctl32  crypt32  dwmapi  gdi32  kernel32  ntdll
ole32  oleaut32  shell32  shlwapi  user32  userenv  ws2_32
```

По сравнению с замером 19 сентября добавились `bcrypt`, `crypt32`, `userenv`, `ws2_32` и
`combase` (последний принесла подписка на падение рендерера через COM) —
их принесли хранилище учётных данных, сетевой слой аватаров и проверка сертификатов у
обновлений. Все четыре входят в саму Windows, вывод раздела 3 не меняется:
`VCRUNTIME140.dll` по-прежнему не нужен.

- [x] Зависимости релизного бинарника — только системные DLL
- [x] `cogit.exe` в одиночестве в пустой папке запускается и отрисовывается
- [ ] **Прогон в свежей виртуальной машине не сделан.** На машине разработчика
      Windows 11 Home, где Windows Sandbox недоступен, а другой чистой системы нет

Что проверять в виртуальной машине, по порядку:

1. Windows 10 без свежего Edge — `Cogit_0.1.0_x64-setup.exe` должен докачать WebView2
2. Та же машина, portable `cogit.exe` без установщика: окно **не откроется**, и это
   ожидаемо. Проверяется, что это видно пользователю, а не молчаливый выход
3. Учётная запись без прав администратора — установка в профиль пользователя
4. Путь с пробелами и кириллицей
5. `Cogit_0.1.0_x64_en-US.msi` через `msiexec /qn` — сценарий корпоративной раскатки

## 8. SmartScreen: почему он ругается и что с этим делать

При первом запуске скачанной сборки на чужой машине Windows Defender SmartScreen
показывает «Система Windows защитила ваш компьютер» и прячет кнопку запуска под
«Подробнее». Это происходит и с `cogit.exe`, и с `.msi`, и с `-setup.exe`.

**Это не баг Cogit.** SmartScreen не проверяет, что внутри файла; он спрашивает у
репутационной службы Microsoft, знает ли она этот файл и этого издателя.

### Как считается репутация

| Что оценивается | Как набирается |
|---|---|
| Репутация **файла** | по хешу: сколько раз этот конкретный файл скачали и запустили без жалоб |
| Репутация **издателя** | по подписи Authenticode: переносится с файла на файл и с версии на версию |

Неподписанный бинарник репутации издателя не имеет вовсе, и каждая новая сборка
начинает с нуля — исправил опечатку, пересобрал, снова предупреждение.

**Обычный сертификат (OV) против EV:**

| | Обычный (OV) | EV |
|---|---|---|
| Что подтверждает | что издатель — существующее юрлицо | то же плюс расширенная проверка |
| Хранение ключа | файл `.pfx` или хранилище Windows | обязательно аппаратный токен или облачный HSM |
| Репутация | **набирается постепенно** — от сотен до тысяч установок, недели | **с первого файла**, предупреждения нет сразу |
| Цена в год | заметно дешевле | заметно дороже |

Практический вывод: с OV-сертификатом предупреждение исчезнет не сразу, а после того как
им подпишут несколько выпусков и их наберётся достаточно скачиваний. С EV — сразу.
Ускорить набор репутации на OV можно, отправив сборку в Microsoft через
«Submit a file for malware analysis», но гарантии это не даёт.

### Как включить подпись в сборке

Обвязка уже готова и **необязательна**: без сертификата сборка проходит, просто без
подписи. Подпись включается наложением конфигурации (см. раздел 5), а в CI — секретами:

```yaml
# Ключ и пароль приходят из секретов, в репозитории их нет.
- name: Import the signing certificate
  shell: pwsh
  run: |
    $bytes = [Convert]::FromBase64String($env:WINDOWS_CERT_BASE64)
    [IO.File]::WriteAllBytes("$env:RUNNER_TEMP\cogit.pfx", $bytes)
    Import-PfxCertificate -FilePath "$env:RUNNER_TEMP\cogit.pfx" `
      -CertStoreLocation Cert:\CurrentUser\My `
      -Password (ConvertTo-SecureString $env:WINDOWS_CERT_PASSWORD -AsPlainText -Force)
  env:
    WINDOWS_CERT_BASE64: ${{ secrets.WINDOWS_CERT_BASE64 }}
    WINDOWS_CERT_PASSWORD: ${{ secrets.WINDOWS_CERT_PASSWORD }}

- name: Build
  run: npm run tauri build -- --config src-tauri/tauri.signing.conf.json
  env:
    TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_UPDATER_KEY }}
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ""

- name: Remove the certificate
  if: always()
  shell: pwsh
  run: Remove-Item "$env:RUNNER_TEMP\cogit.pfx" -Force -ErrorAction SilentlyContinue
```

`certificateThumbprint` в `tauri.signing.conf.json` подписывает **все три** артефакта:
бандлер подписывает `cogit.exe` до упаковки, а затем оба установщика. Подпись кода и
подпись обновлений — разные вещи и разные ключи: первая для Windows, вторая для
`tauri-plugin-updater`.

### Что сказать тестировщику, пока сертификата нет

> Windows покажет синее окно «Система Windows защитила ваш компьютер». Нажмите
> **«Подробнее»**, затем **«Выполнить в любом случае»**.
>
> Если кнопки «Подробнее» нет или файл не запускается молча, снимите с него метку
> «скачано из интернета»: правой кнопкой по файлу → **Свойства** → внизу вкладки
> «Общие» галочка **«Разблокировать»** → OK. То же самое из PowerShell:
>
> ```powershell
> Unblock-File .\Cogit_0.1.0_x64-setup.exe
> ```
>
> Разблокировать нужно тот файл, который вы скачали. Если это установщик, то после
> установки предупреждения на `cogit.exe` уже не будет.

## 9. Какой артефакт кому

| Артефакт | Кому и когда | Чем отличается |
|---|---|---|
| `Cogit_0.1.0_x64-setup.exe` (NSIS) | обычная установка одним человеком | ставится в профиль пользователя, без прав администратора; докачивает WebView2, если его нет; умеет обновляться через `tauri-plugin-updater` |
| `Cogit_0.1.0_x64_en-US.msi` (WiX) | развёртывание по сети, групповые политики, SCCM/Intune | ставится для всей машины, требует прав администратора; знает Windows Installer — откат, восстановление, `msiexec` |
| `cogit.exe` | переносимый вариант, флешка, проверка без установки | ничего не ставит и не пишет в реестр; **WebView2 не докачивает** — на машине без него окно просто не откроется |

Тихая установка:

```powershell
# NSIS: /S — тихо, /D — папка (всегда последний аргумент, без кавычек)
.\Cogit_0.1.0_x64-setup.exe /S /D=C:\Tools\Cogit

# MSI: /qn — без интерфейса, /norestart — не перезагружать, лог в файл
msiexec /i Cogit_0.1.0_x64_en-US.msi /qn /norestart /l*v install.log

# MSI: удаление тем же способом
msiexec /x Cogit_0.1.0_x64_en-US.msi /qn
```

Обновления через приложение (Help ▸ Check for Updates…) устанавливают тот же формат,
которым Cogit был поставлен. Для MSI-развёртывания по политике автообновление лучше
оставить выключенным и обновлять централизованно: иначе приложение и политика будут
спорить за одну и ту же установку.

## 10. Linux: пакет, иконка, WSLg

Окно под WSLg получает иконку приложения только если оболочка находит запись `cogit.desktop`
с `Icon=cogit` и окно сообщает класс, который запись называет. Иначе WSLg рисует на панели
задач Windows пингвина.

| Что | Как |
|---|---|
| Класс окна | `g_set_prgname("cogit")` в `run()` до запуска GTK: Wayland `app_id` и X11 `WM_CLASS` берутся из имени программы, а не из `identifier` (`identifier`, `Cogit`; Tauri отдаёт его `GApplication` только при `enableGTKAppId`, по умолчанию выключено — R-696) |
| Иконка окна | иконка по умолчанию Tauri (первый `.png` в `bundle.icon` — теперь `128x128@2x.png`, 256 px) уходит в `_NET_WM_ICON` всем окнам; `gtk::Window::set_default_icon_name("cogit")` добавляет имя из темы |
| `.deb` | `bundle.linux.deb.files` кладёт готовую `src-tauri/linux/cogit.desktop` (`StartupWMClass=cogit`, `Icon=cogit`) в `/usr/share/applications`, `hicolor/{16…256}` и `scalable/cogit.svg` — в `/usr/share/icons`. Бандлер сам пишет `Cogit.desktop` (по `productName`); `desktopTemplate` = `linux/Cogit-alias.desktop` прячет его через `NoDisplay=true` ([R-693](12-risks.md)) |
| Portable | `cogit --install-desktop-entry` / `--uninstall-desktop-entry` ([F-640](features/F-640-linux-desktop-entry.md)): те же файлы в `~/.local/share` |
| Исходники иконок | `src-tauri/icons/linux/hicolor`: SVG нарисован заново по `icon.png` (векторного оригинала в репозитории нет), PNG отрисованы из него `rsvg-convert`; вшиты в бинарник через `include_bytes!` |

После установки под WSL выполнить `wsl --shutdown` в Windows: WSLg читает иконки при запуске
дистрибутива. Запись попадает и в меню Пуск, в папку дистрибутива.

```bash
npm run tauri build -- --ci --bundles deb --config '{"bundle":{"createUpdaterArtifacts":false}}'
dpkg-deb -c target/release/bundle/deb/*.deb | grep -E 'applications|icons'
```

У AppImage в Tauri 2 опции `desktopTemplate` нет: внутри лежит стандартная запись (`Icon=cogit`, `Exec=cogit`, без `StartupWMClass`), класс окна `cogit` совпадает с её именем и `Exec`. Для WSLg его запускают как portable: `--install-desktop-entry` ставит запись с `Exec`, равным `$APPIMAGE` (сам файл образа, а не временная точка монтирования).

## 11. Portable-сборка (feature `portable`)

Portable — отдельная сборка того же кода, а не режим, который приложение определяет на лету: в `src-tauri/Cargo.toml` есть cargo-feature `portable` (по умолчанию выключен). Бинарник, собранный с ним, **знает**, что он portable; установщики и пакеты (NSIS, MSI, deb) собираются без feature и ведут себя как раньше. Ни маркерного файла, ни переменной окружения, ни флага командной строки нет. Сборка: `npm run tauri build -- --no-bundle --features portable` (CLI Tauri v2: `-f, --features`).

| Платформа | Артефакт | Как собирается (`release.yml`) |
|---|---|---|
| Windows | `Cogit_<версия>_x64_portable.zip`: `Cogit/cogit.exe`, `LICENSE`, `README.txt` | первым шагом, `--no-bundle --features portable`; потом установщики **без** feature (cargo пересобирает `cogit`, так и задумано) |
| Linux | `Cogit_<версия>_amd64.AppImage` | вторая сборка на Linux-раннере: после `--bundles deb` идёт `--bundles appimage --features portable --config plain` (без артефактов апдейтера, поэтому `latest.json` больше не содержит запись `linux-x86_64`) |

### Где лежат данные

Корень данных — папка **`Cogit-data`** рядом с бинарником (`crates/portable`, чистая функция `binary_folder`). Рядом означает: настоящая папка файла после разворачивания симлинков (у Windows снимается префикс `\\?\`). У AppImage исполняемый файл живёт в read-only точке монтирования squashfs, поэтому берётся папка файла `.AppImage` из `$APPIMAGE`; если переменной нет (кто-то запустил «голый» portable-бинарник), — папка самого исполняемого файла.

| Подпапка | Что внутри | Чем задаётся |
|---|---|---|
| `config/` | `settings.json`, `presets/`, `.window-state.json` (тот же формат, что у `tauri-plugin-window-state`) | явный путь: `AppContext.config_dir`, `portable_window_state` |
| `local/` | профиль WebView2 / WebKitGTK (`localStorage`, cookies, кэш движка); на Linux ещё `XDG_DATA_HOME` | `data_directory` **каждого** окна (главное создаётся в `setup` из конфига с `create: false`, дочерние — в `child_window.rs`): один профиль на все окна, как и раньше |
| `logs/` | `cogit-<время>.log`, `panic.log` | явный путь в `logging::init` |
| `tmp/` | временные копии сторон внешнего merge tool (`cogit-merge/…`), `cogit-view`, список лицензий, скретч-файлы git | `TEMP`/`TMP` (Windows) или `TMPDIR` (Linux) процесса |
| `cache/` | кэш аватаров (`cache/avatars`); на Linux `XDG_CACHE_HOME` | явный путь + `XDG_CACHE_HOME` |

### Как перенаправляется запись

Сначала проверялась идея «выставить `APPDATA`/`LOCALAPPDATA` и XDG-переменные в начале `run()`». На Windows она **не работает**: `dirs` 6.0 и Tauri 2.11 спрашивают у оболочки известные папки (`SHGetKnownFolderPath`) и переменные `APPDATA`/`LOCALAPPDATA` игнорируют; настроить абсолютный путь профиля WebView2 для окна из `tauri.conf.json` нельзя (Tauri принимает там только относительный). Поэтому:

- **Windows:** явные пути для каждого писателя (таблица выше) + `TEMP`/`TMP` процесса → `tmp/`. `tauri-plugin-window-state` в portable не подключается: он при сохранении делает `create_dir_all(app_config_dir())` — пустая папка в `%APPDATA%` всё равно была бы системной записью; вместо него `portable_window_state` (тот же формат файла, плюс проверка достижимости от `window_place`).
- **Linux:** то же самое явное и ещё XDG: `XDG_CONFIG_HOME`→`config`, `XDG_DATA_HOME`→`local`, `XDG_CACHE_HOME`→`cache`, `XDG_STATE_HOME`→`local/state`, `TMPDIR`→`tmp`, чтобы GLib, WebKitGTK, GTK (недавние файлы, диалог выбора файла) и `dirs` писали в `Cogit-data`. К `XDG_CONFIG_DIRS` и `XDG_DATA_DIRS` спереди дописываются настоящие домашние папки пользователя: темы, ассоциации файлов и `.desktop`-файлы продолжают находиться на чтение.
- **Переменные ставятся один раз**, самым первым действием `run()` (процесс однопоточный, `set_var` небезопасен в многопоточном; единственный `unsafe` вынесен в `portable::set_var`).

**Что видят дочерние процессы.** git, ssh, credential helper'ы, хуки, внешний merge tool, терминал, файловый менеджер и `xdg-open` должны вести себя как при обычном запуске, поэтому исходные значения XDG-переменных (или их отсутствие) сохраняются до подмены и возвращаются в окружение каждого запускаемого процесса: `git_engine::runner::clear_inherited_git_vars` (единственный путь сборки команд git, хуков и их оболочек), `app_state::merge_tool::spawn`, `app_state::desktop::command_for`. Функция `portable::redirect` чистая и покрыта тестами. **Исключение — временная папка:** `TEMP`/`TMP`/`TMPDIR` дети видят перенаправленными, иначе скретч-файлы git оказались бы в системной временной папке, то есть это те самые записи, которых пользователь не хочет. На Windows других переменных не подменяется, так что детям возвращать нечего. Процессы, которые запускает плагин opener из JS (`openUrl`, `revealItemInDir`), окружение не восстанавливают (хука нет): на Windows это `ShellExecute` и Проводник, на Linux `xdg-open`, которому хватает дописанных `XDG_*_DIRS`.

### Что portable не делает

- **Миграция старых папок** (`legacy_dirs`) пропускается целиком.
- **Автообновление отключено:** плагин updater не регистрируется, `Check for Updates` объясняет, как обновиться (заменить файл), проверка при запуске не выполняется, в About строка `Updates` пишет `Portable build: replace it to update`.
- **Не доступна для записи папка** (read-only носитель, нет прав): запуск **прерывается** (`portable::Error::NotWritable`, сообщение в stderr, код выхода 1), без окна сообщения. Откат на системные папки или на временную папку не делается — portable не должен молча писать в систему. Папки создаются при первом запуске; проверяется запись пробным файлом, повторный запуск ничего не ломает.
- **`--install-desktop-entry`** работает как раньше и пишет в `~/.local/share`: это явное действие пользователя.

### Решение для пользователя: токены HTTP

Токены HTTP хранит `keyring` — это Windows Credential Manager / Secret Service, то есть **системное хранилище**, и portable-режим их не трогает: молча класть секреты в обычный файл нельзя. Альтернатива (не сделана): шифрованный файл рядом с бинарником с парольной фразой пользователя (Argon2 + XChaCha20-Poly1305, фраза спрашивается при первом обращении). Минус: ещё один диалог и забытая фраза означает потерю токенов. Решение за пользователем, см. [R-698](12-risks.md).

### Неустранимые записи вне папки

Приложение их не контролирует; список честный, а не «ноль записей»:

- Windows: рантайм WebView2 (Evergreen) ведёт своё состояние в `HKCU`/`ProgramData` и обновляется сам; Windows пишет для окна AppUserModelID и записи списка переходов панели задач, `Recent` для открытых документов, Defender/SmartScreen добавляют `Zone.Identifier` к скачанному exe; Credential Manager (см. выше); `cogit.exe`, запущенный из `%TEMP%` архиватором, лежит там, где его распаковали.
- Linux: AppImage-рантайм сам монтирует образ в `/tmp/.mount_*`; GTK-диалог выбора файла может записать в `dconf` через `gsettings`, если демон его доступен (файл `dconf` лежит вне `XDG_*`).
- Оба: то, что делает git по настройкам пользователя (глобальный конфиг, `credential.helper`, ssh-ключи в `~/.ssh`) — см. выше, дети видят обычное окружение.

### Как проверить вручную (приложение при разработке не запускалось)

Автоматически проверено: чистые функции (`crates/portable`, тесты на временной «папке бинарника» проверяют, что ни один путь вне `Cogit-data` не возвращается и ничего вне неё не создаётся), окружение детей на уровне `Command`. **Трассировки настоящего приложения нет**, её нужно сделать самому:

- Windows: Process Monitor (Sysinternals), фильтры `Process Name` is `cogit.exe`, затем отдельно `msedgewebview2.exe` (дети `cogit.exe`: правило `Parent PID`), `Operation` is `WriteFile`/`CreateFile` с правом записи/`RegSetValue`/`RegCreateKey`, `Path` not begins with путь к `Cogit-data`. Всё, что осталось, сверить с разделом выше.
- Linux: `strace -f -e trace=openat,mkdir,rename,unlink -o /tmp/cogit.trace ./Cogit.AppImage`, затем `grep -E 'O_WRONLY|O_RDWR|O_CREAT|mkdir' /tmp/cogit.trace | grep -v '<папка Cogit-data>'`; для живой картины `inotifywait -m -r ~/.config ~/.local/share ~/.cache /tmp` во время работы приложения. Запись в `/tmp/.mount_*` и `/dev/shm` ожидаема.
- В обоих случаях открыть About: строка `Portable data` показывает папку и открывает её.
