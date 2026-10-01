# Cogit — выпуск релиза

Сборка и публикация идут через GitHub Actions: `.github/workflows/ci.yml` проверяет
каждый push и PR, `.github/workflows/release.yml` собирает и публикует установщики.
Что именно получает пользователь — [13-distribution.md](13-distribution.md).

## Как выпустить релиз

1. Поднять версию **во всех манифестах одним коммитом**: `Cargo.toml`
   (`[workspace.package]`), `package.json`, `frontend/package.json`. `Cargo.lock`
   обновится сам (`cargo check`).
2. Повесить тег и отправить его:

   ```bash
   git tag v1.2.3
   git push origin v1.2.3
   ```

3. Дождаться `Release` в Actions. Релиз создаётся **черновиком с пустым описанием**
   (`RELEASE_DRAFT: "true"` в начале `release.yml`). Заметки не генерируются нигде:
   открыть черновик на GitHub (**Releases ▸ Edit**), написать описание в Markdown,
   проверить файлы и нажать **Publish release**. Пока черновик не опубликован, он не
   виден в блоке «Releases» справа. Не ставьте `"false"`: релиз выйдет с пустым описанием.
4. Перезапуск для того же тега безопасен: **Actions ▸ Release ▸ Run workflow**, поле
   `tag`. Существующий релиз не пересоздаётся, его описание и состояние не трогаются,
   файлы заменяются (`gh release upload --clobber`).

Формат тега: `vMAJOR.MINOR.PATCH` или с суффиксом `-rc.1`, `-beta.2`. Суффикс — релиз
помечается как pre-release.

## Версия

Источник правды — манифесты в репозитории, тег обязан с ними совпасть. Шаг
`scripts/ci/check-version.mjs` сверяет тег с `Cargo.toml`, обоими `package.json` (и
`tauri.conf.json`, если там появится `version`) и **падает с понятной ошибкой** при
расхождении. Автоматическая подмена версии в сборке не выбрана намеренно: тогда
собранные файлы не соответствовали бы содержимому тегнутого коммита.

Ограничение WiX: в MSI допустима только числовая метка пре-релиза (`v1.2.3-1`, не более
65535). Для `-rc.1` и `-beta.2` собирается **только NSIS**, MSI пропускается.

## Что публикуется

| Файл | Что это |
|---|---|
| `Cogit_<версия>_x64.msi` | WiX, установка на весь компьютер, для корпоративной раскатки |
| `Cogit_<версия>_x64-setup.exe` | NSIS, установка в профиль пользователя, без прав администратора |
| `Cogit_<версия>_x64_portable.zip` | Windows portable: папка `Cogit` с `cogit.exe`, `LICENSE`, `README.txt` (без маркерных файлов); ничего не устанавливается, нужен WebView2; данные в `Cogit-data` рядом с exe |
| `Cogit_<версия>_amd64.deb` | Linux, пакет с ярлыком и иконками |
| `Cogit_<версия>_aarch64.dmg` | macOS (Apple Silicon), экспериментальная сборка без подписи |
| `Cogit_<версия>_amd64.AppImage` | Linux portable: один файл, без установки; данные в `Cogit-data` рядом с файлом; без автообновления |
| `*.sig` | подпись автообновления (только если задан `TAURI_UPDATER_KEY`) |
| `latest.json` | манифест, который читает автообновление приложения (тоже только с подписью) |
| `SHA256SUMS.txt` | контрольные суммы всех файлов релиза |

- **WebView2:** `bundle.windows.webviewInstallMode = downloadBootstrapper` — установщики
  докачивают рантайм, если его нет. Автономный вариант (+130 МБ) — `offlineInstaller`.
- **UpgradeCode MSI:** задаётся Tauri по имени продукта и не меняется между версиями,
  поэтому новая версия обновляет старую на месте. Не переименовывайте `productName`.
- **Установка:** NSIS — на пользователя, MSI — на машину (у WiX иначе нельзя).
- **Portable — отдельная сборка того же кода** с cargo-feature `portable` (`--features portable`), без маркерного файла и без режимов во время работы: бинарник знает, что он portable (данные в `Cogit-data` рядом, миграция и автообновление выключены, подробности — [13-distribution.md](13-distribution.md) §11). Установщики и deb собираются без feature.
- **Portable для Windows** собирается до установщиков, без сборщика пакетов и с feature
  (`tauri build --no-bundle --features portable`): сборщик вписывает в `cogit.exe` тип пакета
  (nsis/msi), и портативная копия выглядела бы установленной. Установщики потом собираются без
  feature (cargo пересобирает `cogit`, так и задумано). При наличии сертификата exe подписывается
  `signtool`. Автообновление портативной версии отключено.
- **AppImage — портативная Linux-сборка:** на раннере две сборки, сначала `--bundles deb` (обычная), потом
  `--bundles appimage --features portable` без артефактов апдейтера; поэтому `latest.json` не содержит
  `linux-x86_64`.
- **Linux** (`linux-x64`) — обычное задание: его сбой блокирует релиз. AppImage собирается с
  `APPIMAGE_EXTRACT_AND_RUN=1` и `NO_STRIP=true` (на раннере нет FUSE, `strip` спотыкается о
  новые секции ELF).
- **macOS** (`macos-arm64`, Apple Silicon) — **`experimental: true`**: сбой не блокирует релиз,
  `.dmg` в нём просто не будет. Сборка не подписана и не нотаризована, поэтому Gatekeeper при
  первом запуске предупредит: открыть через правую кнопку ▸ Open или
  `xattr -dr com.apple.quarantine /Applications/Cogit.app`. Автообновления на macOS нет
  (собирается без артефактов апдейтера). Portable-сборки для macOS нет. Подпись и нотаризация —
  отдельные секреты Apple, здесь не настроены.
- **Описание релиза** пишется вручную; в `release.yml` нет `--generate-notes`, и файла
  `.github/release.yml` с категориями нет. Автогенерации не должно быть.

## Секреты (Settings ▸ Secrets and variables ▸ Actions)

Все необязательны: без них сборка проходит, но без подписи.

| Секрет | Для чего |
|---|---|
| `WINDOWS_CERT_BASE64` | PFX сертификата подписи кода, в base64: `[Convert]::ToBase64String([IO.File]::ReadAllBytes("cert.pfx"))` |
| `WINDOWS_CERT_PASSWORD` | пароль от PFX |
| `TAURI_UPDATER_KEY` | **содержимое** приватного ключа автообновления (`%USERPROFILE%\.cogit\cogit-updater.key`), не путь |
| `TAURI_UPDATER_KEY_PASSWORD` | пароль ключа; пусто, если ключ без пароля |

Сертификат импортируется в хранилище раннера, отпечаток передаётся в
`bundle.windows.certificateThumbprint`; Tauri подписывает `cogit.exe` и оба установщика,
метка времени — DigiCert. Подпись кода и подпись автообновления — разные ключи.

## Как добавить ещё платформу

1. Добавить запись в матрицу `release.yml` (и `ci.yml`): `os`, `arch`, `experimental`.
2. Задать `bundles` в выражении `BUNDLES` и ветку в шагах «Build installers and packages» и
   «Collect and rename».
3. Для Linux на другой архитектуре — системные пакеты в `.github/actions/setup`.
4. Часть WebView2-специфики (`webview2.rs`, `#[cfg(windows)]`) на других платформах
   не применяется, см. R-620 в [12-risks.md](12-risks.md).
