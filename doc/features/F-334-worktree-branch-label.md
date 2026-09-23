# F-334 · Метка ветки, взятой в другой worktree

Метка локальной ветки в графе, выписанной в другом worktree, начинается с иконки worktree (та
же, что в панели Worktrees), а тултип называет путь и состояние: `clean`, `modified` или
`missing`. Ветка worktree, открытого сейчас, не помечается — она HEAD. Проверить: добавить
worktree на ветку `feature`, вернуться в основной — у метки `feature` иконка и тултип
`Checked out in worktree <путь> (clean)`; изменить файл в worktree — `(modified)`.
