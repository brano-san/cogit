# F-565 · Bisect: старт, отметки, Reset

По образцу SmartGit ([Bisect](https://docs.syntevo.com/SmartGit/Latest/Manual/GUI/Bisect): `Branch | Bisect | Start`, красная точка — плохой, зелёная — хороший, `Mark as Good` в меню коммита, `Mark HEAD as Bad` / `Mark HEAD as Good`, в конце — `Bisect Finished` с `Copy ID`, `Leave Bisect`, `Continue Bisect`; `Abort` в баннере). `Skip` у SmartGit в документации нет — он из `git bisect skip`.

- [ ] Старт — диалог `Start Bisect` с полями `Bad commit` и `Good commit`: из подменю `Bisect ▸ Start…` коммита в графе (плохой — HEAD, хороший — этот коммит; на самом HEAD хороший пуст), из `Branch ▸ Bisect ▸ Start…` и палитры `Start Bisect…` (хороший — выбранный в графе коммит). Под полем — найденный коммит или «No commit is called …». Хороший можно оставить пустым и отметить позже.
- [ ] `Mark as Good`, `Mark as Bad`, `Skip` — для любого коммита из подменю `Bisect ▸` графа, для HEAD — из баннера, `Branch ▸ Bisect` и палитры; git сразу выписывает следующий коммит на проверку. Уже отмеченный так же — пункт отключён с причиной.
- [ ] `Reset` (`git bisect reset`) — там же и `Abort Operation In Progress`; до конца поиска спрашивает, фокус на Cancel: метки пропадут, Undo их не вернёт. В журнале безопасности — строка без Undo.
- [ ] Bisect, начатый в терминале с `--term-new/--term-old`, отмечается его словами.
- [ ] Проверить в сборке: на истории из 16 коммитов с ошибкой в 11-м — старт из меню первого коммита, отметки из баннера, через 4–5 шагов найден 11-й; `Skip` на выписанном коммите выписывает соседний; `Start…` отключён во время merge с причиной.
