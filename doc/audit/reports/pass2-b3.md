# Проход 2 · b3 — S-24…S-27 и провода App (36 коммитов, HEAD af561aa)

Medium: FR-009 25dae90 (R-520; проверить двойной клик по submodule и по закрытой строке) · FG-012 96bc0bd (прокрутка к найденному уже была; файл на Working Tree; старый тест закреплял баг)
bug: DF-043 0586afe · FR-016 bb4edf2 · FR-017 c560c5d · FR-018 600d5b2 · FR-041 db0cc66 (проверить A → submodule → Close на A) · FR-044 dc813c8 (проверить 3 папки) · FG-033 6a51f46 · FG-056 23553d3 · FG-058 e149111 · FG-066 147021a · FS-014 3a3d1e6 · FS-023 d299fcd · FS-032 ed4ef26
race: CC-008 1a76549 (switch_with_autostash, stash_pop_oid; R-521, 04; autostash.rs ×5; stashes.push удалён)
inconsistency: FR-024 c5872cb · FR-027 86ba8bc (проверить счётчик) · FR-028 81de903 (до 4) · FR-038 a969694 (R-354 к коду) · FR-039 3524f8a · FG-037 47375f2 (A/B files.toggle-tree)
ux: FR-026 81d2562 · FG-057 787ae14 (проверить «onto abc1234^») · FG-065 1b5356b (проверить) · FS-016 e7cc5aa · FS-020 7630d13 (проверить Ctrl+Shift+F)
Провода: FX-009 6374270 (проверить behind, ahead, notInitialised) · отметки по секциям 66b976b · dirty Hooks/Add Tag/Stash All/Stash Selection 2c0169b + 263211b + f5f1ab3 · «… all» в заголовке Unstaged 7c7060e + d1aedc3 · меню каретки закрывается с кнопкой b561742 · F6 в панели слияния af561aa (11 §7)
Не делал: DC-001 кнопка и FX-026 провод (b1 не был слит).
UX не сделано: Push сбрасывает выбор (afterRefChange → как afterFetch); Fetch отдельной кнопкой в detached HEAD; состояние merge/rebase неактивной строки из pulse; подсказка при двойной неудаче автостэша.
Тесты: vitest 2401/2401; nextest git_engine autostash/stashes/stash_modes/branches 44/44; cogit lib 124.
Окружение: build-b3 держал пути build-a5 — заменены.
Конфликты: App.svelte, HooksPanel, src-tauri lib.rs, lib/ipc/index.ts, bindings, 04, 12-risks (R-520, R-521, правки R-354, R-43), app_state stashing.rs, commands/branches.rs.
