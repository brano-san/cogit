# F-640 · Иконка и запись в меню под Linux / WSLg

Окно Cogit под WSLg получает иконку приложения вместо пингвина: у окна класс `cogit` (`g_set_prgname` до GTK), иконка передана окну напрямую, а запись `cogit.desktop` с `Icon=cogit` и `StartupWMClass=cogit` ставится пакетом `.deb` или командой `cogit --install-desktop-entry`.

| Команда | Что делает |
|---|---|
| `cogit --install-desktop-entry` | `~/.local/share/applications/cogit.desktop` (Exec — путь к текущему бинарнику) и `icons/hicolor/{16,24,32,48,64,128,256}x…/apps/cogit.png` + `scalable/apps/cogit.svg`; учитывает `$XDG_DATA_HOME`, если он абсолютный; повторный запуск перезаписывает те же файлы |
| `cogit --uninstall-desktop-entry` | удаляет ровно эти девять файлов, чужое не трогает |
| `cogit --version`, `--help` | печатают и выходят, окно не открывается |

Код — крейт `desktop_entry` (без Tauri), иконки — `src-tauri/icons/linux/hicolor`, вшиты в бинарник. После установки под WSL нужен `wsl --shutdown` из Windows. См. [13-distribution.md](../13-distribution.md) §10, [R-693](../12-risks.md).
