# F-650 · Заголовок, меню-бар и контекстные меню в веб-слое

Под WSLg нет рабочего стола, и GTK рисует светлые рамку и меню. Страница рисует их сама.

## Что видит пользователь

- **Linux:** окна без системной рамки; сверху строка Cogit — иконка, меню-бар (главное окно), название окна, Minimize / Maximize|Restore / Close. Перетаскивание за пустое место, двойной щелчок — maximize, восемь захватов по краям и углам для изменения размера.
- **Меню:** Repository … Help, Alt+буква или F10 открывают, стрелки / Enter / Esc / набор первых букв, наведение переключает открытые меню. Активные пункты — `fg.primary`, `fg.disabled` только у неактивных; клавиши справа; галочки у переключателей View.
- **Контекстные меню:** открываются сразу, подменю — по наведению (150 мс) или →, разворачиваются у края окна.
- **Windows / macOS:** без изменений (нативное). Переключатель: Preferences ▸ Advanced ▸ Window ▸ «Draw menus in the window» (`uiWebMenus`: Automatic / Always / Never, нужен перезапуск); Always на Windows — для проверки.

## Устройство

| Часть | Файл |
|---|---|
| Решение «кто рисует» (чистая функция, тест) | `src-tauri/src/window_chrome.rs`, команда `window_chrome` |
| Дерево меню из тех же таблиц, что нативное | `src-tauri/src/menu/model.rs`, команда `menu_model` |
| Клик → тот же путь, что у нативного события | команда `menu_command` → `dispatch_menu_command` |
| Окна без рамки, скрытая нативная строка | `lib.rs` (setup), `child_window.rs`, `menu::rebuild` |
| Рамка окна каждого вида | `WindowFrame.svelte` (точка входа каждого окна), `Titlebar.svelte`, `ResizeEdges.svelte` |
| Меню-бар и список строк | `MenuBar.svelte`, `MenuSurface.svelte` (общий для контекстных меню и выпадающих) |
| Контекстное меню | `ContextMenu.svelte`, фасад `popupContextMenu` в `lib/ipc/index.ts` |
| Чистая логика (TDD) | `lib/menu-nav.ts` (клавиши, набор букв, mnemonic), `lib/popup-place.ts` (`placeAtPoint`, `placeSubmenu`, `placeDropdown`), `lib/titlebar.ts`, `lib/web-menu.ts`, `tidy` в `lib/context-menu.ts` |
| Состояние | `stores/web-menus.svelte.ts` |

Решения и отклонения — [R-695](../12-risks.md). Клавиши — [11 §13](../11-keybindings.md).
