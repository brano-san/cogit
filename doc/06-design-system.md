# Cogit — Дизайн-система

> Источник истины для всех визуальных решений. Цвета — `frontend/src/themes/`, остальное — `frontend/src/app.css`.
> Раскладка и поведение — [05-ui-layout.md](05-ui-layout.md).

## 1. Правила

1. **Ни одного цвета в коде компонента.** Только `var(--...)`. Литерал (`#hex`, `rgb()`, `hsl()`,
   именованный цвет в цветовом свойстве, `color-mix` с `transparent`) вне `frontend/src/themes/`
   ломает `npm --prefix frontend run check:colors` (pre-commit и CI).
2. **Цвета — это токены тем.** Один JSON на тему, имена по смыслу (`badge.tag.bg`), не по оттенку.
   Компоненты читают токены или алиасы из `aliases.css`; о теме они не знают.
3. **Четыре темы** — Light, Light gray, Dark gray, Dark (`data-theme` = `light`,
   `lightGrey`, `darkGrey`, `dark`; #24, R-234). Файлы `light.json`, `light-gray.json`,
   `dark-gray.json`, `dark.json`.
4. **Сплошные цвета.** Никаких alpha-наложений: каждый оттенок задан под каждую тему отдельно.
   Исключение одно — `focus.ring`. Затемнение за модальным окном — сплошной `overlay.scrim`,
   рисуемый с `opacity: var(--scrim-opacity)`; тень — сплошной `shadow.color`, размытие само
   сводит его на нет.
5. **Шкалы, а не произвольные числа.** Отступы — из шкалы 4 px, размеры шрифтов — из шкалы типографики.

## 2. Токены цвета

### Архитектура

| Что | Где |
|---|---|
| Список токенов с типом и описанием | `frontend/src/themes/tokens.schema.json` |
| Значения | `themes/light.json`, `light-gray.json`, `dark-gray.json`, `dark.json`: `{ name, family, tokens }` |
| Загрузчик | `lib/theme.ts` (чистые функции), `stores/theme.svelte.ts` (применение), `lib/theme-colors.ts` (кэш цветов для canvas) |
| Старые имена | `frontend/src/aliases.css` — `--surface-*`, `--state-*`, `--text-*`, `--status-*`, `--c-*` указывают на токены |
| Не цвет | `app.css`: шрифты, отступы, высоты, радиусы, движение |
| Охрана | `scripts/check-colors.mjs`, исключения — `scripts/check-colors.allow` (пустой по замыслу) |

Имя токена — путь через точки. Токен становится CSS-переменной на `:root`: точки превращаются в
дефисы, camelCase-сегменты переводятся в kebab-case:
`diff.add.word` → `--diff-add-word`, `bg.selectedInactive` → `--bg-selected-inactive`,
`fg.onAccent` → `--fg-on-accent`, `graph.lane.3` → `--graph-lane-3`. Правило одно и проверяется
тестом (`theme.test.ts`: два токена не могут дать одну переменную).

Типы в схеме: `color` — сплошной `#rrggbb`; `color-alpha` — `#rrggbb` или `#rrggbbaa` (только
`focus.ring`); `string` — не переменная CSS. `syntax.theme` — имя синтаксической темы,
**зарезервировано**: Shiki не подключён, подсветка — Lezer, поле пока ничего не читает.

### Как грузится тема

1. До первой отрисовки: Vite-плагин `cogit-boot-theme` кладёт в `<head>` каждого окна
   `--boot-bg`/`--boot-fg` четырёх тем (значения читает из `themes/*.json`) и крошечный
   скрипт, ставящий `data-theme` из `localStorage` (`cogit.theme`). Окно не открывается белым.
2. `src/boot.ts` (первый импорт каждого окна) вызывает `bootTheme()`: применяет тему прошлого
   запуска — все токены становятся inline-свойствами `:root` до монтирования.
3. `settings.load()` читает файл настроек, `themeStore.apply(theme)` ставит выбранную тему,
   `themeStore.loadUser()` читает `user-theme.json` и накладывает его.
4. Смена темы — те же `setProperty` на `:root`, без перезагрузки. Дочерние окна
   (diff, blame, investigate, merge, commit) получают её как раньше: `followSettings` →
   `settings.reload()` → `apply`.
5. Canvas (граф, навигация investigate) не читает `var()`: `tokenColor('--graph-line')` спрашивает
   `getComputedStyle` один раз на смену темы и кэширует; `themeStore.version` перерисовывает холст.

### Валидация и fallback

`resolveTheme` проверяет каждый токен схемы. Отсутствующий или невалидный токен — предупреждение
(`warn` в `cogit.log`, контекст `theme`, один раз на текст) и значение из базовой темы того же
семейства: `light`, `lightGrey` → `light`; `dark`, `darkGrey` → `dark`. Если и базовая его не
имеет, переменная остаётся незаданной.

### user-theme.json

Необязательный файл `user-theme.json` в каталоге конфигурации приложения (там же, где
`settings.json`; Windows: `%APPDATA%\dev.branosan.cogit\`). Формат:

```json
{ "tokens": { "accent": "#e5484d", "bg.editor": "#101216" } }
```

Накладывается поверх выбранной темы, любой из четырёх; ключи, которых нет, берутся из темы.
Неизвестный токен, невалидное значение (не `#rrggbb`, alpha вне `focus.ring`) и неверная форма
файла — предупреждение в лог и игнорирование, приложение стартует. Читает файл команда
`read_user_theme` (логика — `app_state::settings::read_user_theme`). UI пока нет: будущий экран
настроек будет писать в этот файл.

### Как добавить новый цвет

1. Добавить токен в `themes/tokens.schema.json`: имя по смыслу, `type`, описание.
2. Добавить значение во **все четыре** файла тем (`light`, `light-gray`, `dark-gray`, `dark`).
   Сплошной `#rrggbb`; серые темы подбираются отдельно, не как «светлая с прозрачностью».
   Тест `theme.test.ts` падает, если ключи файлов и схемы расходятся.
3. Нужно ли старое имя компонентам? Только тогда — алиас в `aliases.css`. Новому коду алиас не нужен.
4. Использовать как `var(--<имя>)` (имя — по правилу выше). Для canvas — `tokenColor('--<имя>')`.
5. `npm --prefix frontend run check:colors` и `npx vitest run` (`design-tokens.test.ts` ловит
   необъявленные переменные, `theme.test.ts` — валидность и контраст).

### Поверхности

| Токен | Применение |
|---|---|
| `bg.app` | Хром: тулбар, меню, статус-бар, шапки панелей, подложка окна |
| `bg.panel` | Сайдбары и списки: Repositories, Worktrees, Branches, Files, детали коммита |
| `bg.editor` | Рабочая поверхность: граф и diff (`<Panel surface="editor">`). Самая светлая в светлых темах |
| `bg.elevated` | Попапы, дропдауны, тултипы, диалоги, тосты |
| `bg.input` | Поля ввода, чекбоксы, блоки вывода |
| `bg.hover` | Наведение |
| `bg.selected` / `bg.selectedInactive` | Выбранная строка панели с клавиатурой / без неё |
| `selected.bar` | Полоса 2 px слева у выбранной строки |
| `panel.activeHeader` | Подчёркивание шапки активной панели (2 px) |
| `bg.rowStripe` | Каждая вторая строка графа поверх `bg.editor` |
| `border`, `border.strong` | Разделители (1 px) / границы полей, рамка окна |
| `scrollbar.thumb` | Ползунок прокрутки |

Выбранная строка — `--state-selected` (`bg.selected`) и полоса `--selected-bar`. `Panel` без
фокуса переопределяет `--state-selected` на `bg.selectedInactive`, поэтому строки внутри
панели не знают о фокусе.

### Текст и акцент

`fg.primary`, `fg.secondary` (подписи, заголовки панелей), `fg.muted` (даты, хеши, пути; ≥ 4,5:1 на `bg.editor`, `bg.panel`, `bg.app` — на `bg.elevated`, `bg.hover`, `bg.selected` вместо него `fg.secondary`; тест `theme.test.ts`),
`fg.disabled`, `accent` / `accent.hover` (ссылки, фокус, галочки), `fg.onAccent` (текст на
заливке акцентом), `focus.ring` (контур фокуса, alpha), `toggle.on.bg|fg|border` (включённый
переключатель: фон, текст, рамка).

### Статусы, файлы, бейджи, граф

`status.success|warning|danger|info` (файлы A / M / D / R, `dirty`, ошибки),
`status.move`, `status.stash`, `file.untracked`, `file.conflict`, `badge.merging.fg`,
`badge.head|branch|remote|tag|warning` (`.bg` / `.fg`), `graph.lane.0…7`, `graph.lane.neutral`.
`graph.lane.neutral` — единственный цвет линий и узлов неотмеченных веток (`--graph-line`); `graph.lane.0…7` только для отмеченных веток. Главная линия — `fg.primary` (`--graph-main`). Дорожки и цвета отмеченных веток (`--graph-branch-1…8` = `graph.lane.0…7`) держатся не ниже 3:1 к
`bg.panel` и `bg.selected` (`graph-palette.test.ts`).

### Diff

`diff.add|del|move` × `line|word|gutter` (у `move` нет `gutter`), `diff.hunkHeader.bg|fg`,
`diff.lineNumber`, `diff.centerGutter.bg|action|action.hover`, `diff.connector.fill|stroke`,
`diff.filler.bg|hatch`, `diff.jumpFlash`. Красный — удалено, зелёный — добавлено, фиолетовый — перемещено; единого
цвета «changed» нет (янтарные фон и маркеры убраны). Подробнее — §7.

`diff.filler.bg|hatch` (R-628) — фон и линии штриховки filler-строк режима Aligned:

| Токен | Light | Light gray | Dark gray | Dark |
|---|---|---|---|---|
| `diff.filler.bg` | `#fafbfc` | `#eceef1` | `#2a2c30` | `#181a1e` |
| `diff.filler.hatch` | `#eceef1` | `#e1e3e7` | `#313338` | `#1f2226` |

Контраст у них намеренно низкий (спокойная штриховка), проверка контраста для них не вводилась.

### Дополнительные токены (не из таблиц задания)

Введены, потому что интерфейсу нужен цвет, которого нет в спецификации; значения выведены из
палитры спецификации (смешение двух её цветов), см. `12-risks.md`:
`bg.rowStripe`, `status.move`, `status.stash`, `diff.jumpFlash`, `search.hit`,
`search.current`, `search.ink`, `overlay.scrim`, `shadow.color`.

### Алиасы (`aliases.css`)

Старые имена остаются, но указывают на токены; новый код пишет имя токена.

```css
--surface-base        → --bg-app
--surface-panel       → --bg-panel
--surface-editor      → --bg-editor
--surface-raised      → --bg-elevated
--surface-input       → --bg-input
--state-hover         → --bg-hover
--state-selected      → --bg-selected        (в Panel без фокуса — --bg-selected-inactive)
--state-focus-ring    → --accent
--state-pressed       → --toggle-on-bg       включённый переключатель (`aria-pressed`)
--state-pressed-text  → --toggle-on-fg

--divider             → --border
--field-border        → --border-strong

--text-primary        → --fg-primary
--text-secondary      → --fg-secondary
--text-muted          → --fg-muted
--text-code           → --fg-primary
--link                → --accent

--status-add          → --status-success
--status-modify       → --status-warning
--status-delete       → --status-danger
--status-ref          → --accent
--status-tag          → --badge-tag-fg

--graph-main          → --fg-primary
--graph-line          → --graph-lane-neutral
--graph-branch-N      → --graph-lane-(N-1)   N = 1…8, цвета отмеченных веток
--graph-focus         → --accent             ветка выбранного коммита без своего цвета
--graph-bisect-good   → --status-success     точка хорошего коммита bisect (F-566)
--graph-bisect-bad    → --status-danger      точка плохого и первого плохого
--graph-bisect-skip   → --fg-secondary       точка пропущенного
--graph-bisect-current → --badge-warning-bg  фон строки коммита на проверке
--graph-bisect-found  → --diff-del-line      фон строки первого плохого

--indicator-changes   → --status-warning     незакоммиченные изменения (Repositories, метка worktree)
--indicator-synced    → --status-success     чисто и всё запушено
--indicator-push      → --status-warning     «есть что пушить» (R-548)
--indicator-pull      → --status-success     «есть что забрать»
--indicator-unknown   → --fg-secondary       fetch не удался

--scrim               → --overlay-scrim      рисуется с opacity: var(--scrim-opacity)
--shadow-popover/-dialog                     0 8px 24px / 0 12px 40px, цвет --shadow-color
--c-*                                        прежние примитивы (`--c-bg-panel`, `--c-added-bg`…) → токены
```

`--c-*-bg` теперь сплошные: `--c-added-bg` → `diff.add.line`, `--c-deleted-bg` → `diff.del.line`,
`--c-modified-bg` → `badge.warning.bg`, `--c-branch-bg` → `badge.branch.bg`,
`--c-tag-bg` → `badge.tag.bg`, `--c-stash-bg`, `--c-moved-bg` → `diff.move.line`.

## 4. Типографика

| Роль | Семейство | Размер | Высота строки | Насыщенность |
|---|---|---|---|---|
| UI обычный | Inter, system-ui, sans-serif | 13 px | 20 px | 400 |
| UI плотный (списки) | то же | 12 px | 18 px | 400 |
| Заголовок панели | то же | 11 px | 16 px | 600, `letter-spacing: 0.04em`, uppercase |
| Код и хеши | JetBrains Mono, Consolas, monospace | 12.5 px | 18 px | 400 |
| Diff-редактор | то же | 12.5 px | 18 px | 400 |
| Статус-бар | Inter | 11.5 px | 16 px | 400 |

Шрифты **встраиваются** в приложение (`frontend/src/assets/fonts/`), а не подгружаются с CDN:
десктопное приложение обязано работать офлайн, и мигание подстановки шрифта недопустимо.

Числовые колонки (даты, счётчики, номера строк) — `font-variant-numeric: tabular-nums`,
иначе цифры «пляшут» при обновлении.

## 5. Шкалы

### Отступы (база 4 px)

```
--sp-1: 2px    --sp-2: 4px    --sp-3: 6px    --sp-4: 8px
--sp-5: 12px   --sp-6: 16px   --sp-7: 24px   --sp-8: 32px
```

### Высоты

```
--h-row:      24px   строка списка
--h-row-dense:22px   строка графа
--h-toolbar:  40px
--h-menubar:  28px
--h-statusbar:24px
--h-panel-hdr:28px
--h-input:    26px
```

### Радиусы

```
--r-sm: 3px   бейджи, поля ввода
--r-md: 4px   кнопки, ref-капсулы
--r-lg: 6px   диалоги, всплывающие панели
```

Скруглений больше 6 px в интерфейсе нет — плотный интерфейс с крупными радиусами
выглядит рыхлым.

### Тени

Одна тень на всё приложение, только для всплывающих поверхностей:
`--shadow-popover: 0 8px 24px var(--shadow-color)`; у диалога — `--shadow-dialog` (12 / 40). Цвет — сплошной токен `shadow.color`.
Тени на панелях и кнопках не используются — разделение делают границы 1 px.

## 6. Компоненты

### Строка списка

```
высота          --h-row
padding         0 --sp-5
hover           background: --state-hover
stripe          каждая вторая строка графа, Files, Repositories, Branches, Worktrees —
                --row-stripe, класс .striped по striped(место строки в списке как нарисован,
                не в DOM), до правил hover, selected и menu, которые его перекрывают;
                отметки — box-shadow, видны поверх (F-580)
selected        background: --state-selected
                + ::before — полоса 2px слева цветом --status-ref
focus-visible   outline: 1px solid --state-focus-ring; outline-offset: -1px — только у строки,
                до которой дошли Tab, не выбрав её
menu            строка, на которой открыто контекстное меню (Repositories): background
                --state-hover + outline 1px --state-focus-ring, offset -1px, поверх выбора;
                выбор не меняется (R-545)
```

Список — одна остановка клавиатуры (класс `.key-list` в `app.css`, #27): фокус панели
показывает её заголовок, место в списке — выделенная строка. Поэтому ни сам прокручиваемый
контейнер, ни уже выбранная строка рамку фокуса не рисуют; кнопки и поля внутри строки —
рисуют, как везде.

Индикатор выбора — **полоса, а не только цвет фона**: различие фона `bg.hover` и `bg.selected`
слишком мало (полоса — `selected.bar`), чтобы быть единственным сигналом.

### Таблица Files

Строка — сетка: `Name` `minmax(0, 2fr)`, `Type` `--file-type-width`, `State`
`--file-change-width`, `Path` `minmax(0, 3fr)` (без пути имя — `1fr`), зазор `--file-column-gap`.
Заголовки колонок — `--h-row-dense`, `--fs-header`, `--text-secondary`, у колонки сортировки —
`--text-primary` и общий `Caret` (вверх — по возрастанию). Место полосы прокрутки списка
занято всегда (`scrollbar-gutter: stable`), заголовки отступают на `--scrollbar-size`
([R-595](12-risks.md)).

### Статус-бейдж файла

Квадрат 16×16, радиус `--r-sm`, моноширинная буква по центру, цвет по статусу,
фон — сплошной токен темы (`diff.add.line`, `badge.warning.bg`, …).

| Статус | Буква | Цвет |
|---|---|---|
| Added / Staged | `A` | `--status-add` |
| Modified | `M` | `--status-modify` |
| Deleted | `D` | `--status-delete` |
| Renamed | `R` | `--status-ref` |
| Copied | `C` | `--status-ref` |
| Unmerged | `U` | `--status-delete` |
| Untracked | `?` | `--text-secondary` |

### Ref-капсула

Высота 16 px, радиус `--r-md`, padding `0 --sp-3`, шрифт моноширинный 11 px,
граница 1 px цветом роли, фон и текст — токены `badge.<вид>.bg` / `.fg` (head, branch, remote, tag).

| Тип | Цвет | Пример |
|---|---|---|
| Локальная ветка | `--status-ref` | `dev` |
| HEAD | `--status-ref`, насыщенный фон | `HEAD → dev` |
| Удалённая ветка | `--status-ref` приглушённый | `origin/master` |
| Ветка и её upstream на одном коммите | блок remote `badge.remote.fg` на `badge.remote.bg`, `=`, блок ветки — цветом ветки или HEAD | `origin=dev` |
| Тег | `--status-tag` | `v1.2.0` |
| Stash | `--status-stash`; узел в графе — квадрат того же цвета | `stash@{0}` |

### Кнопка тулбара

Высота 28 px, padding `0 --sp-5`, радиус `--r-md`, прозрачный фон, иконка 14 px + текст.
Hover — `--state-hover`; active — `--state-selected`; disabled — `opacity: 0.4`, курсор по умолчанию.
Кнопка с меню получает `▾` 8 px справа.

### Переключатель

Кнопка с состоянием вкл/выкл — `aria-pressed`, другого признака нет. Включённая — фон
`--state-pressed`, иконка и текст `--state-pressed-text`, при наведении тоже: одно правило
`[aria-pressed="true"]` в `app.css` с `!important`, потому что hover-правило любого компонента
специфичнее глобального селектора, и наведение прятало нажатое состояние (R-592). Неприменимый
переключатель (`aria-disabled`, `.dead`) не бывает нажатым. Фон и цвет нажатого — только из этого
правила; своё у компонента — разве что рамка (Investigate, окно слияния).

### Поле ввода / фильтр

Высота `--h-input`, фон `--surface-input`, граница 1 px `--field-border`, радиус `--r-sm`.
При фокусе граница меняется на `--state-focus-ring`. Плейсхолдер — `--text-secondary`.
Крестик очистки появляется только при непустом значении.

### Галочка и радиокнопка

Только `Checkbox.svelte`, `Radio.svelte` и лёгкая галочка ниже, нативных элементов формы нет:
системный квадрат белый на тёмной панели. Под рисунком — настоящий скрытый input (Space, стрелки,
Tab, экранный диктор). Квадрат 14 px (`--checkbox-size`), фон `--surface-input`, граница
`--field-border`; отмечено и `mixed` — заливка `--status-ref`, галочка или тире цветом
`--surface-base`. Радио — круг того же размера с точкой `--status-ref`. Подпись справа, рамка
выровнена по первой строке подписи. Состояние, вычисленное из модели (галочка заголовка в
Branches), — проп `tri` с `triState` (R-158, [R-455](12-risks.md)).

В списках на сотни строк — лёгкая галочка: класс `tick-box` на самом `<input type="checkbox">`
(`appearance: none`, без компонента, подписи и SVG), тот же рисунок и цвета, `mixed` — тот же
`triState`; размер — `--tick-box-size` на элементе списка (в Branches — 12 px).

### Заголовок панели

Высота `--h-panel-hdr`, фон `--surface-base` (`bg.app`), нижняя граница 1 px `--divider`. Активная панель (с клавиатурой) — подчёркивание 2 px `panel.activeHeader`.
Слева — название в стиле «заголовок панели», справа — фильтр и действия панели.

### Блок терминального вывода

Фон `--surface-input`, шрифт моноширинный, `white-space: pre`, горизонтальная прокрутка,
`user-select: text` обязательно. URL внутри оборачиваются в кликабельные элементы
цветом `--link` с подчёркиванием при наведении.

### Ссылка

Любая ссылка — `<a>` или кнопка, которая ведёт себя как ссылка, — цветом `--link`, без
подчёркивания, с подчёркиванием при наведении. Для `<a>` это делает глобальное правило в
`app.css`, так что браузерный синий не появится нигде; кнопке-ссылке цвет задаётся явно
(R-172).

## 7. Подсветка diff

| Что | Фон | Дополнительно |
|---|---|---|
| Строка добавлена | `diff.add.line`, жёлоб `diff.add.gutter` | Метка `+` в жёлобе цветом `--status-add` |
| Строка удалена | `diff.del.line`, жёлоб `diff.del.gutter` | Метка `−` цветом `--status-delete` |
| Строка перемещена | `diff.move.line` | — |
| Слово добавлено | `diff.add.word` | Радиус 2 px |
| Слово удалено | `diff.del.word` | Радиус 2 px |
| Filler-строка (Aligned) | `diff.filler.bg`, штриховка `diff.filler.hatch` | Тонкая диагональ 1 px, плитка 6 px; без номера, знака и текста (R-628) |
| Плашка сворачивания | `--surface-raised` | Текст `--text-secondary`, по центру |
| Маркер конфликта | `diff.del.line` | Левая полоса 2 px `--status-delete` |

Подсветка синтаксиса накладывается **поверх** фона diff и не должна им перебиваться:
тема Lezer настраивается с прозрачным фоном.

## 8. Движение

| Что | Длительность | Кривая |
|---|---|---|
| Hover, смена цвета | 80 мс | `ease-out` |
| Раскрытие аккордеона | 120 мс | `ease-out` |
| Появление диалога | 140 мс | `cubic-bezier(0.2, 0, 0, 1)` |
| Перетаскивание сплиттера | 0 | без анимации |
| Вспышка изменения после перехода в Diff (F-541) | 600 мс | `ease-out`, гаснет; ничего её не ждёт |
| Обновление списка данными | 0 | без анимации |

Анимации длиннее 150 мс в инструменте разработчика воспринимаются как тормоза.
При `prefers-reduced-motion: reduce` все переходы отключаются; вспышка изменения горит без
затухания то же время.

## 9. Иконки

Единый набор линейных иконок, штрих 1.5 px, сетка 16×16, выравнивание по пиксельной сетке.

Размеры: главный тулбар — 22 px; кнопки в тулбарах панелей (Repositories, Files, Branches) —
один токен `--panel-icon` (16 px) в кнопке `--h-button-sm` и крупнее; значок вида строки в
дереве — `--kind-icon` (14 px). Стрелки push и pull на углах значка строки Repositories —
`--sync-arrow` (10 px, линия 2 px, двойной ореол цвета панели), выступают за правый край
значка на 5 px; значок держит 2 px справа, чтобы они не касались точки изменений (R-547).
Иконки — inline SVG с `currentColor`, никаких иконочных шрифтов и никаких растровых иконок.
Для символов Git (ветка, коммит, тег, стэш, слияние) используется собственный минимальный набор,
согласованный по оптической плотности с Inter.

## 10. Чек-лист перед мержем UI-изменений

- [x] Ни одного литерального цвета — только токены
- [x] Отступы взяты из шкалы `--sp-*`
- [x] Высоты строк соответствуют `--h-*`
- [x] Выделение показано не только цветом (полоса, иконка или подпись)
- [x] Фокус с клавиатуры виден
- [x] Числовые колонки используют `tabular-nums`
- [ ] Проверено при 125% и 150% масштабирования системы
- [x] Пустое состояние панели оформлено
- [x] Поведение при очень длинном имени файла/ветки — обрезка с многоточием и тултипом
