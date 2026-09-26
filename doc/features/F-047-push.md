# F-047 · Push с защитой

- [ ] Кнопка Push отправляет коммиты, а принудительная отправка всегда идёт как --force-with-lease.
- [ ] Первый push ветки, у которой нет upstream, публикует её в remote под тем же именем и назначает upstream (`--set-upstream <remote> HEAD`), как сделал бы `push.autoSetupRemote`, — вместо отказа git «has no upstream branch».
- [ ] Ветка без upstream при нескольких remotes: Push (тулбар, палитра, меню ветки) открывает
      Push To — выбрать remote, `Set upstream` включён (R-551). Upstream — это `branch.<имя>.remote`
      и `.merge` вместе, как у git: ветке с одним `merge` Push тоже ставит upstream.
- [ ] В отсоединённом HEAD Push и Push To… неактивны («HEAD is not on a branch»); ветка без
      upstream отправляется (`--set-upstream`, R-414).
