# F-333 · Состояние репозитория в панели Graph и в Repositories

Баннер незавершённого merge, rebase, cherry-pick, revert, bisect или `git am` и баннер
detached HEAD стоят в панели Graph над списком коммитов (с кнопками Continue/Skip/Abort),
строка Working Tree дописывает состояние (`Working Tree (1 conflicted), merging`), а в
Repositories рядом с именем репозитория и на узле сабмодуля — метка цветом предупреждения
(`<merging>`, `<rebasing>`, `<cherry-picking>`, `<reverting>`, `<bisecting>`,
`<applying patches>`, у репозитория ещё `<detached>`). Проверить: начать merge с конфликтом —
баннер над графом, `, merging` в строке Working Tree, `<merging>` в Repositories; Abort
возвращает всё как было.
