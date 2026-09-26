# F-563 · Запомненные фильтры графа

- [ ] Лупа в поле фильтра графа открывает меню: `Remember Pattern` — запомнить текст поля (неактивен с причиной, если поле пустое или текст уже запомнен), `Forget Pattern` — забыть текст поля, если он запомнен; ниже — запомненные шаблоны, новые сверху, не больше 30. Выбор шаблона подставляет его в поле и сразу фильтрует; `✕` в строке шаблона или `Delete` на нём забывает шаблон. Список сохраняется в настройках (`graphFilterPatterns`) и виден в `Preferences ▸ Graph & History ▸ Filter`, где шаблон тоже можно забыть.
- [ ] Проверить в сборке: ввести `author:brano fix`, лупа ▸ `Remember Pattern`; очистить поле, лупа ▸ выбрать шаблон — поле заполнено, граф отфильтрован; `✕` у шаблона — пропал из меню и из Preferences; меню с клавиатуры: `Enter` на лупе, `↑`/`↓`, `Delete`, `Esc`.

Справочно: SmartGit — «You can save common search patterns for later usage by clicking on the search icon … and selecting Remember Pattern (similarly, Forget Pattern will remove the current pattern if it has been saved previously)», https://docs.syntevo.com/SmartGit/Latest/Manual/GUI/Repository/Repositories-Directories-and-Files.
