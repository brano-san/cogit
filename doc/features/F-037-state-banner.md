# F-037 · Баннер состояния

- [ ] Прерванный merge, rebase, cherry-pick или revert показывает баннер над рабочей областью: у rebase, cherry-pick, revert и `git am` — `Continue`, `Skip`, `Abort`; у merge только `Abort` — коммит слияния делается обычным коммитом из панели Commit (R-572). Проверить в сборке: merge с конфликтом — в баннере нет `Continue`, текст зовёт закоммитить; cherry-pick с конфликтом — `Continue` есть.
- [ ] `Abort` и `Skip` (кнопки баннера и `Abort Operation In Progress` в палитре) сначала спрашивают, фокус на Cancel: Undo их не отменяет. `Continue` выполняется сразу.
