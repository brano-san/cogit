# F-635 · Пустое состояние окна, нативная тема, шрифты

Без репозитория окно показывает одну карточку «No repository open» с кнопками `Open…`, `Clone…`, `Welcome…` (те же потоки, что в палитре); панели пусты, без текста. Карточка не мигает при открытии и до конца восстановления сессии (`emptyStateVisible`, `repository.ready`).

Тема Cogit определяет `color-scheme` страницы и (Linux) `gtk-application-prefer-dark-theme` — [06 «Нативная тема»](../06-design-system.md), [R-692](../12-risks.md). Шрифтовые стеки `--font-ui` / `--font-mono` содержат Linux-запасные гарнитуры.
