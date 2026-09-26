# Поиск репозиториев в папке

Scan Folder обходит выбранную папку параллельно и предлагает открыть все найденные репозитории списком с чекбоксами.
Cancel и новый скан останавливают идущий обход (`cancel_operation`), а не только прячут его находки.
Тесты — `crates/git_engine/tests/discover.rs` `a_cancelled_scan_stops_between_hits`, `scan.test.ts`
«stops the walk when the dialog is cancelled».
