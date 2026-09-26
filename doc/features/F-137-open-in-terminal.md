# F-137 · Открыть в терминале

`Open in Terminal` в меню репозитория (F-360) открывает терминал в корне репозитория. Какой —
выбирается в Preferences ▸ Tools & Integrations ▸ CLI & Terminal ▸ `Open in Terminal uses`:
`System default` (на Windows — Command Prompt в новом окне), `Windows Terminal`, `PowerShell`,
`Command Prompt`, `Git Bash`; на Linux и macOS выбора нет — терминал системы. Путь передаётся
одним аргументом только программе, которая не наследует рабочую папку (`wt -d`). Запуск — тот же
`desktop::spawn`, что у Open Git Shell и PowerShell: консольные PowerShell, Command Prompt и Git Bash
идут через скрытый `cmd /C start`, в своём окне, а не в консоли `tauri dev` (R-261). Тесты —
`crates/app_state/tests/terminal.rs`. Проверить: выбрать `Git Bash`, `Open in Terminal` на
репозитории с пробелом в пути — Git Bash открыт в его корне.
