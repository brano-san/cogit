# F-360 · Контекстное меню репозитория

Правый клик по репозиторию в Repositories: `Open Repository`, `Open in Explorer` (сама папка),
`Reveal in Explorer` (родитель с выделенной папкой), `Open in Terminal`, `Open in PowerShell`
и `Open in Git Shell` (только Windows), `Close Repository`, —, `Pull`, `Push`, —, `Move To ▸`
(группы панели), `Pin`, `Rename…`, `Remove…`. У строки с меткой `missing` — открытой или
закрытой — `Open Repository` и всё, что открывает папку, выключены с причиной. Проверить:
`Open in Explorer` открывает окно самой папки репозитория, `Move To ▸ <группа>` переносит
строку в группу; закрыть репозиторий, удалить папку, перезапустить — у строки `missing`
пункты папки выключены. `Close Repository` и `Remove…` строки, чей submodule или worktree
сейчас в панелях, освобождают панели вместе с ним: открыть A, кликнуть его submodule S,
ПКМ по A → `Close Repository` — на экране следующий открытый репозиторий, не S.
