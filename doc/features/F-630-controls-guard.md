# F-630 · Общий Button и защита от нестилизованных контролов

`Add Note…` / `Edit Note…` и `Verify` в панели коммита — общий `Button` (`sm`, высота строки метаданных); подпись по наличию заметки, текст заметки справа в строке `Note` (обрезается, полный в подсказке). `Forget Resolution`, кнопки окна Errors и ✕ истории Blame — тоже `Button`; опция `ConfirmDialog` — общий `Checkbox`; ветка в Push — `RevisionCombobox`.

Защита — [06 «Контролы»](../06-design-system.md), [R-690](../12-risks.md): `appearance: none` и голый вид по умолчанию, тест `lib/controls.test.ts`.
