# F-321 · Remote ▸ Submodule

Подменю `Initialize`, `Synchronize`, `Reset…`, `Add…`, `Deactivate…`, `Deinit…`, `Unregister…`
действует на сабмодуль, открытый в дереве Repositories или выбранный в Files, иначе —
Initialize/Synchronize на все, остальные спрашивают, какой; без сабмодулей активен только
`Add…`. Проверить: в репозитории с сабмодулем `Deinit…` → подтверждение → строка сабмодуля
«not initialized»; `Initialize` возвращает его.
