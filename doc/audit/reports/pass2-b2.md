# Проход 2 · b2 — S-22, S-23 (17 коммитов, 14/14)

FS-010 b76bf4b + 5584ceb (Esc и ✕ спрашивают «Discard Changes», подложка не закрывает, Cancel не спрашивает; dirty у Config, Index Editor, Edit Message, Rebase, Split; «Unsaved Work» при закрытии окна; диалог по глубине модального стека; R-515, F-532; проверить Edit Git Config → текст → щелчок мимо / Esc / закрыть окно)
FS-012 a6e2470 (claimable в lib/keymap.ts; общий lib/accelerator-cases.json с Rust accelerators::parse; клавиша без Ctrl/Alt, кроме F, не перехватывается; AltGr, Win, Ctrl+A/C/V/X/Z/Y/F, Alt+F4 — отказ; R-516, 11 §11)
DF-011 ada4cf2 (DiffView capture с active; F6 за последним изменением — обходу панелей; 11 §2 §7, F-014; проверить F6 из Graph, Ctrl+F в поле коммита)
GE-009 f7ede72 (полное имя — только при неоднозначности; и в drop ветки на ветку) · FX-033 f90ac1b · FR-022 6004ace (lib/ref-checkout.ts; checkoutTag удалён) · FR-029 39e113e (R-517) · FS-035 e7e83d3 (shortcutOf) · FS-040 48bb3ae (событие cogit://settings-changed страницы; R-518, 04 §6; проверить тему при открытом Blame) · FR-023 9a0019c · FG-028 f63093d + 921aa51 (проверить 4 темы, Enter, Tab, перетаскивание в Rebase) · FG-019 e9a0069 · FS-030 9784024 (в detached неактивна и каретка Pull) · FS-034 77cb5eb
Правленные тесты: keymap.test блок accelerator → recordKeys (без модификатора — отказ); toolbar.test Push в detached → branch: true; refAt с tags и fullName.
Удалено: accelerator(), checkoutTag в App.
UX не сделано: HooksPanel dirty; dirty у AddTagDialog и Stash All; «… all» в заголовке Unstaged по правилу строк; Fetch в каретке неактивного Pull; Sync для ветки без upstream; событие настроек типом в Rust; F6 в панели слияния.
Тесты: vitest 2332/2332 (184 файла); cogit lib 124; svelte-check 0.
В App проведено всё своё. Конфликты: App.svelte, ref-menus.ts, RefActions.svelte, accelerators.rs, menu/mod.rs (с b1), 12-risks, 11, features/README, 04 §6.
