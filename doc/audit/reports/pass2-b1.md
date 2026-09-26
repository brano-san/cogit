# Проход 2 · b1 — S-19, S-20, S-21 + DC-001 (29 коммитов, HEAD 44a318f)

Medium: FX-026 bb0335b (Set Upstream…/Stop Tracking в #33 и #39 под Push To…; R-505, F-130, 04; проверить диалог) · DC-001 6158c28 (cancel_network; R-506, F-529, 03, 04)
Кнопка отмены (b3): `cancelNetwork(op.id)` из `$lib/ipc`; op из `operation-changed`/`listOperations()` с phase "running" и kind fetch|pull|push (Push To и Push Up To — push); false — нечего отменять; true — git остановлен, исходный вызов отклоняется CogitError kind "cancelled" (уведомления пропускают), в журнал «Cancelled by the user» (output.warning). network.running id очереди не несёт — kind из события.
bug: FR-042 2a2c74c (ScanChunk started{id}) · FX-005 ed369bd (ChangeKind::Index, working_state.indexLock; R-507) · FX-020 625df2d (PresetStatus.searched) · BE-036 9693fe8 (R-508) · TS-001 08d11d8 · TS-004 01e8fed (git_command_in, user_git_command) · TS-028 fc52900 (content.rs → ls-files -z)
race: CC-023 f68c12d (R-509; проверить «cogit stopped» в логе) · BE-012 baebeaa (без теста) · CC-007 97bdbb3 (R-510)
inconsistency: GR-004 2d7ee57 · BE-007 d9e4a5a (10k элементов ≤ 1,9 МБ, сериализация 0,2–1,5 мс release, JSON.parse ≤ 3,3 мс → исключение INV-02, R-511; ipc_payloads.rs) · BE-008 993e7f4 (проверить Open in Terminal: PowerShell, Cmd, Git Bash) · BE-014 252d021 (проверить окно без прыжка) · BE-035 367ec53 (R-512) · BE-018 1f47116 (R-513) · BE-039 ccf1e22 + 44a318f (refs/cogit/backup/<oid>; R-514, F-035, F-040, M6; Discard — stash push + stash drop; ссылку пишет gix — исключение для refs/cogit/*; 2 теста worktree закрепляли баг) · DC-017 уже 011a719 · TS-020 уже 5490757 · TS-003 уже 3b55728 + 5490757 (остаток — строка crates/CLAUDE.md: «the fixtures' git runs with GIT_CONFIG_GLOBAL/GIT_CONFIG_SYSTEM redirected; code under test reads the developer's config»)
ux: BE-013 8d99684 + 6c17886 · BE-016 c7914af · BE-032 d0387fd (F-252; проверить Blame → Alt+X → Exit → нет cogit.exe)
dead-code: DF-034 425a0a1 · BE-009 e283731 · GR-018 fd24764 (search_graph удалён, тесты на build_graph + graph_window) · BE-019 29f295c (ушли 4 теста tracked)
simplify: GE-027 a5bc61e (branches_without_divergence — A/B)
Не делал по заданию: GR-010, BE-024, BE-025.
UX не сделано: кнопка отмены в футере (b3); закрытие submodule при уходе; строки «Cancelled…» британские (t9).
Тесты: после S-21 nextest 2133/2133, 50k 18/18, cogit lib 123; на a5bc61e — всё зелёное, кроме mutation_speed (исправлено 44a318f, затронутые 104/104); vitest 2284; svelte-check 0.
Окружение: скопированный build-a1 → build-b1 держал абсолютные пути build-a1 в */output и root-output — заменены.
Конфликты: 12-risks (перед R-485), 04, 01, src-tauri commands/mod.rs, network.rs, tests.rs, lib.rs; app_state lib.rs (OpenRepo owner, lane), queue.rs; graph_cache.rs, tests graph/graph_view/performance, 07 §9; lib/ipc/index.ts, RefActions, notices, DiffView, HooksPanel; test_fixtures, 09.
