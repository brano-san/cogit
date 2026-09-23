# Frontend (Svelte 5)

## State

- One source of truth per panel: header counts and body read the same state.
- Opening a repository: `Closed → Opening → Open | Failed`; the status always ends
  (success, error, timeout).
- Operation status only in the footer; panels show no status text while loading.
- A panel mounted late reads the store; it never waits for an event that may have fired.
- Shared state through `$state` / `$derived`; mutating a plain object does not update.

## UI

- No `window.prompt` / `alert` / `confirm`, no native form controls. Dialogs use the shared
  modal: close button, `Esc` cancels, `Enter` confirms, primary action rightmost.
- Theme tokens only; check all four themes.
- Context menus: shared component. Inapplicable → disabled, not hidden (hide only what the
  platform lacks). No doubled, leading or trailing separators. Shortcuts on the right.
  Destructive items confirm.
- The webview's own context menu is disabled everywhere.
- One `disabled` state drives icon, label and caret; disabled controls do not animate.
- Disclosure triangles, dropdown carets, tooltips: shared components, one size app-wide.
- Truncation: list rows on the right only; graph branch labels in the middle; secondary
  text before the name; never overlapping. One shared utility.
- British spelling in UI strings (`Colours`, `licence`).

## Tests

Test store logic, virtualization and geometry; not markup.