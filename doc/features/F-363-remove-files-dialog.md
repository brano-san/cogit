# F-363 · Диалог Remove

`Remove…` на отслеживаемых файлах открывает `Remove files from the repository`: таблица
файлов с галочками (`Name`, `Directory`) и `Delete local files`, по умолчанию выключенный —
файлы перестают отслеживаться и остаются на диске (`git rm --cached`); включённый удаляет и с
диска (`git rm`). Проверить: Remove без галочки — файл в Staged как удалённый и в Unstaged как
неотслеживаемый.
