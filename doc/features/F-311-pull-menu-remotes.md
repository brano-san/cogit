# F-311 · Меню Pull: remotes, режим Pull и удаление слитых веток

Меню кнопки Pull показывает Pull, Fetch для каждого remote репозитория (текущий первым,
с пометкой `(current)`), Fetch All, выбор «Pull Uses the Current Remote / All Remotes» и
флажок «Delete Merged Branches after Pull»; выбор сохраняется между запусками. Проверить:
в репозитории с двумя remotes открыть меню Pull — пункты `Fetch 'origin' (current)` и
`Fetch '<второй>'`; включить флажок, удалить на remote влитую ветку, нажать Pull —
локальная ветка удалена, Undo её возвращает.
