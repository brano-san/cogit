# F-567 · Bisect в баннере Graph и в Repositories

- [ ] Во время bisect баннер над графом говорит, что делать: какой коммит проверить (id и тема, если граф её загрузил) или какой метки не хватает (`Waiting for a good commit…`), и куда вернёт Reset. Кнопки — `Mark as Good`, `Mark as Bad`, `Skip` (для HEAD) и `Reset`; `Continue` и `Abort`, которых у `git bisect` нет, не показываются.
- [ ] В Repositories у репозитория и узла submodule — метка `<bisecting>`, как `<merging>` и `<rebasing>`; строка Working Tree в графе дописывает `, bisecting`.
- [ ] Bisect из терминала — `git bisect start` без коммитов, `good`, `bad`, `reset` — баннер и метки меняются сами: наблюдатель слышит `BISECT_*` как смену HEAD, в linked worktree — в его git-каталоге.
- [ ] Проверить в сборке: `git bisect start` в терминале — баннер `Mark a bad commit and a good one`; `git bisect bad`, `git bisect good <старый>` — баннер называет выписанный коммит; `git bisect reset` — баннер и `<bisecting>` уходят.
