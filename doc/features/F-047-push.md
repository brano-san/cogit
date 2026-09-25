# F-047 · Push с защитой

- [ ] Кнопка Push отправляет коммиты, а принудительная отправка всегда идёт как --force-with-lease.
- [ ] Первый push ветки, у которой нет upstream, публикует её в remote под тем же именем и назначает upstream (`--set-upstream <remote> HEAD`), как сделал бы `push.autoSetupRemote`, — вместо отказа git «has no upstream branch».
