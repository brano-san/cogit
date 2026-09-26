# F-320 · Remote ▸ Synchronise

`Remote ▸ Synchronise` (`Ctrl+Shift+S`) делает Pull, затем Push текущей ветки в основной remote
(порядок меняет `Sync ▸ Push, then Pull` тулбара, R-224); если Pull не удался, Push не
запускается. Проверить: ветка опережает и отстаёт на один коммит без конфликтов.

- Preferences ▸ Pull = «Merge the remote branch in»: `Ctrl+Shift+S` — после него `ahead 0,
  behind 0`, на ветке merge-коммит.
- Pull по умолчанию (только fast-forward): `Ctrl+Shift+S` ничего не запускает и в уведомлении
  пишет, что ветки разошлись (`1 ahead, 1 behind`), Pull только перематывает, и что сделать —
  слить или перебазировать ветку либо разрешить Pull merge (R-456).
