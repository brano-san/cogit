# F-107 · Каталог пресетов хуков

- [ ] Tools → Manage Hooks → Presets предлагает готовые хуки (cargo fmt, Conventional Commits, отказ от больших файлов, поиск секретов), честно показывая, если нужный инструмент не найден, и где Cogit его искал.
  Подсказка к «<tool> not found» — `installHint` и `Searched: <папки пресета>, PATH` (`PresetStatus.searched`).
  Тесты — `crates/git_engine/tests/presets.rs` `a_missing_tool_names_every_place_that_was_searched`,
  `crates/app_state/tests/presets.rs` `a_preset_with_a_tool_says_where_the_tool_was_looked_for`,
  `preset-tool.test.ts`.
