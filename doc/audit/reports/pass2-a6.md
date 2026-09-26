# Проход 2 · a6 — G-03, G-04 (27 коммитов)

bug: GR-014 fc53b36 · FG-023 уже 89d7cc5 · FG-026 3a0096d (местная полночь; 2 теста закрепляли UTC — переведены) · FG-063 ef68b3a · FS-018 37a9d27 · DC-014 0cfcc46 (12–48 px; тест ≤40 → 48) · GR-002 b45372d · GR-005 0895f0b (без теста — область блокировки; A/B repo.close) · GR-007 5e7e79d (graph_footprint, set_graph_cache_budget) · GR-016 ccb34f0 + ebfdcfd (R-495)
race: GR-017 5a6c75c · CC-020 4056e56 (#load требует открытый репозиторий; фикстуры graph/graph-view открывают его) · FG-040 0742a40
inconsistency: FG-054 c1797d7 · GR-023 ff8cfa1 (индекс без копий oid не делал — горячий путь)
ux: GR-006 84bd41e · GR-012 2f87ad2 · GR-022 bdb4b1d · FG-022 959de8c (R-496; refLabelText удалён с тестом, truncateMiddle оставлен) · FG-024 b846c23 · FG-032 3edc58b · FG-041 0918245 · FG-043 9c30beb · GR-019 939c29e (R-497)
dead-code/split: GR-009 e624f93 (ушли 5 тестов first_parent_* — проверяли только удалённый ViewFilter::first_parent) · GR-011 86ae679 · FG-038 3ec669d (CommitRow; ширины колонок inline из COLUMN_WIDTH; CommitList ~780 строк)
Проверить в сборке: FG-054 author:nobody → Working Tree + «No commits match the filter» + Clear Filter, все галочки сняты → «No branches shown»; FG-022 метка 40+ → одно «…»; FG-032 Relative тикает; FG-043 stash в Files → Working Tree не выделен; GR-012 кольцо HEAD после Compare with HEAD; FG-040 правый клик по строкам прежнего репозитория; GR-019 stash -u + галочка = одна строка; FS-018 Add → Separator в конце неактивна.
A/B: b45372d (lanes/paint), 0895f0b (repo.close), 5e7e79d (repo.switch), ccb34f0 (снятие фильтра, память), ff8cfa1 (PaintMemo::refresh), 939c29e (ByTime/stream_rows), 86ae679, 3ec669d (graph.scroll/first-screen), мелкие 84bd41e, 2f87ad2.
Тесты: vitest 176/2196; nextest graph_engine+git_engine 1298; app_state 442 + performance 7; clippy, svelte-check 0.
Конфликты: CommitList, stores/graph*, graph-overlay, graph_cache.rs, lib.rs (graph: Arc<RwLock>), search.rs, graph_walk.rs, 12-risks, 07, App.svelte, SettingsPanel, truncate.ts.
