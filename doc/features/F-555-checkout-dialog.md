# F-555 · Один диалог Checkout

- [ ] `Check Out` в меню ветки, тега или коммита (Branches и граф) открывает диалог `Check Out`
      с кнопками `Cancel` и `Checkout` (R-560, R-561):
      - локальная ветка в Branches — что она отслеживает и насколько разошлась, чекбокс
        `Don't show again` (дальше — сразу checkout; вернуть — Preferences ▸ Behavior ▸
        Don't show again или флажок Preferences ▸ General ▸ Check Out);
      - remote-ветка — `Create local branch` с именем без remote и `Track remote branch`
        (включён), `Don't create a local branch (just read-only)` — detached HEAD; если есть
        локальная ветка, чей upstream — эта ветка, третий вариант, выбранный сразу:
        `Checkout and fast-forward local branch '<имя>'` (когда она только отстаёт) или
        `Check out local branch '<имя>'` (когда двигать нечего или ветки разошлись);
      - тег и коммит — `Create local branch` (без Track) или read-only, выбран read-only;
      - метка локальной ветки в графе — те же два и `Check out local branch '<имя>'`.
- [ ] Двойной клик по коммиту в графе (и по линиям его строки) и по метке ветки или тега
      открывает тот же диалог, что `Check Out` их меню; по метке текущей ветки — ничего (R-561).
- [ ] Одноимённая локальная ветка без этого upstream не считается отслеживающей: имя
      подсвечено с объяснением, `Checkout` неактивен до смены имени (R-560).
- [ ] Отказ git из-за локальных изменений — то же предложение stash, что при переключении
      (F-132), для любого варианта.
      Тесты — `ref-checkout.test.ts` («checkoutOffer»), `checkout-flow.test.ts`,
      `crates/git_engine/tests/checkout_choices.rs`.
