# F-551 · Меню remote в Branches

Правый щелчок по узлу remote (`origin`, `upstream`…): `Push To…` (Push To текущей ветки с этим
remote), `Pull`, `Fetch`, `Fetch More` (все ветки и теги сервера мимо refspec; ничего нового —
уведомление «Nothing new», R-553), `Rename…`, `Delete`, `Copy URL`, `Set Depth…` (только в
неглубоком клоне), `Properties…` (URL и `Perform background poll or fetch`, R-554), `Toggle`
(R-555). Сетевые пункты отменяются кнопкой в футере. Проверить с двумя remotes: `Fetch` у
`upstream` получает только его ветки; `Rename…` `upstream` → `fork` — узел и отметки веток
переезжают; `Delete` спрашивает и убирает узел; клон `--single-branch` ▸ `Fetch More` приносит
остальные ветки, повторно — «Nothing new»; клон `--depth=1` ▸ `Set Depth…` `10` — в графе 10
коммитов.
