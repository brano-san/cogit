# Проход 2 · a7 — G-17, G-18 (21 коммит)

bug: TS-007 e7c8ba8 (scripts/bench/check.mjs + check.test.mjs; `node --test "scripts/bench/*.test.mjs"`) · TS-008 15a6cb8 (наборы без конфига машины, autocrlf=false; ПЕРЕСОЗДАТЬ наборы `npm run bench:repos`, net.* и submodules до правки не сравнивать; doc/15 §3) · TS-012 889eebf (#[ignore] с отсылкой к graph.full-layout large; R-504, doc/15 §5) · TS-025 fa24f28 · FX-036 7ffc9fb (R-500) · FX-037 уже 742e16f · FX-038 5658d03 (+строка в R-115) · FX-039 уже 082168b
race: TS-027 69b8501 (свои tempdir)
inconsistency: GR-020 9ffd126 · DC-009 0fc8942 (R-501) · DC-010 ec3ffe6 (R-502, документ к коду) · DC-011 05e8fff (R-503) · DC-016 f28d251 · DC-018 a1dd6b1 (F-114, F-117, F-019→F-260, F-030, F-040, F-261→F-450, F-263) · DC-019 a1074a4 (F-137, строка F-136 убрана) · DC-027 974fad4 · DC-029 531555a (каталог фич одной таблицей по номеру) · TS-010 f3f8c3a · TS-013 eac0892 · TS-014 e12b77d · TS-017 b976607
dead-code: TS-021 c930078 ([profile.quick] удалён)
UX-предложения: Retry в баннере index.lock; обводка родителей/потомков при наведении; дата в тултипе капсулы; одно имя Open Log Folder / Reveal Log File. Замечено: doc/15 §2 пишет `reset --keep`, код после c235f43 — `--hard`.
Тесты: nextest 168/168; node --test бенчмарка 6/6; хук.
Конфликты: features/README (пересортирована), 12-risks (R-500…504, строка R-115), doc/15, run.mjs, nextest.toml, 01, 03, 07, файлы фич.
