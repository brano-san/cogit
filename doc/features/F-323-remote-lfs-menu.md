# F-323 · Remote ▸ LFS

Подменю `Install`, `Track…`, `Lock`, `Unlock`, `Prune…`; без `git lfs` активен только
`Install`, и он показывает, где взять Git LFS. Проверить: `Track…` при выбранном
`art/cover.psd` предлагает `*.psd`, после OK в `.gitattributes` строка
`*.psd filter=lfs diff=lfs merge=lfs -text`.
