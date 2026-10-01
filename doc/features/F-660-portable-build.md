# F-660 · Portable-сборка: данные рядом с бинарником

Zip для Windows и AppImage для Linux собираются с cargo-feature `portable` (`npm run tauri build -- --no-bundle --features portable`). Такой бинарник не пишет в систему: настройки, пресеты, положение окна, логи, профиль WebView2 / WebKitGTK, кэш аватаров и временные файлы лежат в папке `Cogit-data` рядом с `cogit.exe` (у AppImage — рядом с файлом `.AppImage`, по `$APPIMAGE`).

| Что | Как |
|---|---|
| Режим | только feature при сборке; нет флага, переменной и файла-маркера; установщики и пакеты собираются без feature |
| Папка | `Cogit-data/{config,local,logs,tmp,cache}`; создаётся при первом запуске; нет записи — запуск прерывается (код 1, stderr), без отката на системные папки |
| Перенаправление | Windows: явные пути, `TEMP`/`TMP`, `data_directory` каждого окна; Linux: ещё `XDG_*`, `TMPDIR`; git, ssh, хуки и merge tool видят исходные `XDG_*` |
| Отключено | миграция старых папок, автообновление (Check for Updates объясняет), `tauri-plugin-window-state` (вместо него свой файл того же формата) |
| UI | About ▸ Files ▸ Portable data (путь и открыть папку), строка Updates, диагностика |
| Остаётся в системе | токены HTTP в `keyring`, состояние рантайма WebView2, AppUserModelID/Recent, `Zone.Identifier` — см. [13-distribution.md](../13-distribution.md) §11 |

Код — крейт `portable` (без Tauri), `src-tauri/src/portable_mode.rs`, `portable_window_state.rs`. Решения и ограничения — [R-698](../12-risks.md).
