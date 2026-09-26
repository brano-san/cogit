# F-552 · Фоновая проверка — по remote

`Properties…` remote ▸ `Perform background poll or fetch`: выключенный флажок исключает сервер
этого remote из фоновой проверки Repositories (стрелки pull); интервал — общий, в Preferences.
Флажок хранится в секции remote (`remote.<имя>.cogitBackgroundFetch=false`), переживает
`Rename…` и уходит с `Delete` (R-554). Проверить: выключить у `origin`, дождаться интервала —
строка не спрашивает сервер (в Output нет `ls-remote`).
