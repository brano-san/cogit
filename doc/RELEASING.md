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

3. Дождаться `Release` в Actions. По умолчанию релиз создаётся **черновиком**
   (`RELEASE_DRAFT: "true"` в начале `release.yml`): проверить файлы, поправить заметки,
   нажать Publish. Поставить `"false"` — публиковаться будет сразу.
4. Перезапуск для того же тега безопасен: **Actions ▸ Release ▸ Run workflow**, поле
   `tag`. Существующий релиз не пересоздаётся, его заметки и состояние не трогаются,
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
| `*.sig` | подпись автообновления (только если задан `TAURI_UPDATER_KEY`) |
| `latest.json` | манифест, который читает автообновление приложения (тоже только с подписью) |
| `SHA256SUMS.txt` | контрольные суммы всех файлов релиза |

- **WebView2:** `bundle.windows.webviewInstallMode = downloadBootstrapper` — установщики
  докачивают рантайм, если его нет. Автономный вариант (+130 МБ) — `offlineInstaller`.
- **UpgradeCode MSI:** задаётся Tauri по имени продукта и не меняется между версиями,
  поэтому новая версия обновляет старую на месте. Не переименовывайте `productName`.
- **Установка:** NSIS — на пользователя, MSI — на машину (у WiX иначе нельзя).

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

## Как добавить Linux и macOS

1. В `ci.yml` и `release.yml` раскомментировать записи матрицы и указать `bundles`
   (`deb,appimage` / `dmg`).
2. Linux: шаг `apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev
   libayatana-appindicator3-dev patchelf` в `.github/actions/setup`.
3. В `release.yml` `--bundles` и переименование сейчас рассчитаны на Windows (`msi`,
   `nsis`): вынести список пакетов и имена в поля матрицы.
4. macOS: подпись и нотаризация — отдельные секреты Apple, здесь не настроены.
5. Часть WebView2-специфики (`webview2.rs`, `#[cfg(windows)]`) на других платформах
   не применяется, см. R-620 в [12-risks.md](12-risks.md).
