# F-017 · Бинарные и большие файлы

- [ ] Бинарный, слишком большой файл или изображение показывают сводку вместо попытки отрисовать текст.
- [ ] Файл ровно в 1 000 000 байт: «File size exceeds the limit of 1,000,000 bytes», под ним
      Old version / New version — размер в байтах и полный id объекта; файл на байт меньше —
      строками (R-531).
- [ ] Текстовый файл, в шестую позицию первой строки которого вписан символ `0x02`: «File is
      considered as binary: invalid character 0x02 in line 1, at position 6 (new version)».
- [ ] `.gitattributes` с `*.dat binary` и `*.lock -diff`: причина — «.gitattributes marks it
      binary» / «… -diff». С `*.txt text` файл с `0x02` показан строками.
- [ ] Удалённый бинарный файл: New version — «Not there: the file is deleted», не `0 bytes`.
- [ ] Изменённый файл рабочего дерева: id новой версии совпадает с `git hash-object <файл>`.
- [ ] Удалённый текстовый файл — все строки удалены; удалённое изображение — «Deleted · N KB»,
      справа «Deleted», а не `N KB → 0 B` (R-534).
