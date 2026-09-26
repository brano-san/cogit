# F-321 · Remote ▸ Submodule

Подменю `Initialise`, `Synchronise`, `Reset…`, `Add…`, `Deactivate…`, `Deinit…`, `Unregister…`
действует на сабмодуль, открытый в дереве Repositories или выбранный в Files, иначе —
Initialise/Synchronise на все, остальные спрашивают, какой; без сабмодулей активен только
`Add…`. Проверить: в репозитории с сабмодулем `Deinit…` → подтверждение → строка сабмодуля
«not initialised»; `Initialise` возвращает его.
