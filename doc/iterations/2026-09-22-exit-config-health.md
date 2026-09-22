# Итерация: выход, git config, Branches, открытие submodules, предупреждения о состоянии

Шесть пунктов. Каждый сначала проверен в текущем коде, затем правлен.

## 1. Таблица по пунктам

| № | Пункт | Статус | Что сделано | Файлы |
|---|---|---|---|---|
| 1 | Подтверждение выхода | сделал с нуля | Диалог Exit на общем `Dialog` (`Esc`/`Enter`), общий шлюз `mayClose` для крестика, `Alt+F4` и нового `Repository ▸ Exit` (`Alt+X`); при незавершённых операциях показывается всегда и перечисляет их; «Don't show again» = `settings.confirmExit` = чекбокс в Settings; единый список подавленного с «Show again» | `components/common/ExitDialog.svelte`, `lib/exit.ts`, `lib/suppressions.ts`, `lib/settings.ts`, `lib/preferences.ts`, `layout/SettingsPanel.svelte`, `src-tauri/src/menu.rs` |
| 2 | Edit Git Config | сделал с нуля | Подменю `Repository… / User…`; путь от git (`rev-parse --git-path config`, `config --global --show-origin`); встроенный редактор с подсветкой INI; проверка `git config --file` до записи, строка ошибки, файл не трогается; CRLF сохраняется; после записи — перечитывание репозитория | `git_engine/src/config_file.rs`, `components/common/ConfigEditor.svelte`, `lib/config-syntax.ts`, `menu.rs` (`Entry::Nested`) |
| 3 | Branches свёрнуты | доработал | Стор хранит раскрытое, а не свёрнутое: новый репозиторий — всё свёрнуто, поздние секции тоже; старые сохранения читаются как были | `stores/refs.svelte.ts`, `App.svelte` |
| 4 | Вложенные submodules | доработал | Две причины найдены и исправлены: путь от не того корня и поиск репозитория вверх; открытие и перечисление сведены в один `module_root` + `open_exact`; ошибка называет причину | `git_engine/src/gitlink.rs`, `app_state/src/lib.rs`, `App.svelte`, `stores/submodules.svelte.ts` |
| 5 | Предупреждения о состоянии | сделал с нуля | Фоновой обход репозитория и всех подмодулей; две обязательные проверки; группировка одинаковых; карточка с `‹ 1 of N ›` (общий компонент с окном ошибок), `Remind me later` / `Ignore for this repository`, крестик = Remind later; подавленное — в списке пункта 1 | `git_engine/src/health.rs`, `lib/health.ts`, `stores/health.svelte.ts`, `layout/HealthNotice.svelte`, `common/QueueNav.svelte` |
| 6 | Метка `diverged` | доработал | Было: `diverged` на любое несовпадение (и тест это закреплял). Стало: `ahead / behind / diverged / unknown` через `merge_base`, с числами и подсказкой-действием | `git_engine/src/submodules.rs`, `lib/module-tree.ts`, `repo-tree/RepositoryList.svelte` |

Попутно: тест `design-tokens.test.ts` нашёл четыре несуществующие CSS-переменные — две
мои из прошлого раунда (окно About без отступов, ссылка без цвета) и две старые (R-152).

## 2. Пункт 4: гипотеза про пути другой ОС

Проверено на копии `E:\Work1\dtv_device` на машине разработки.

**Для этой ошибки гипотеза не подтвердилась.** Падающего пути нет в мире git вообще:

```
$ git -C import/kors/import/libjam/submodules/cmake-useful rev-parse --git-dir
fatal: cannot change to 'import/kors/import/libjam/submodules/cmake-useful': No such file or directory
```

`import/kors/.gitmodules` не содержит `import/libjam`, а `cmake-useful` — подмодуль
**корневого** `import/libjam`, и там с ним всё в порядке:

```
$ cat import/libjam/submodules/cmake-useful/.git
gitdir: ../../../../.git/modules/import/libjam/modules/submodules/cmake-useful

$ git -C import/libjam/submodules/cmake-useful rev-parse --git-dir
E:/Work1/dtv_device/.git/modules/import/libjam/modules/submodules/cmake-useful
```

Все `core.worktree` в `.git/modules/*/config` на этой копии относительные.
`git submodule status --recursive` показывает 20 инициализированных подмодулей, ни одного
с `-`.

**Настоящая причина — в Cogit** (R-149): после открытия `import/kors` путь следующего
подмодуля склеивался от корня `kors`, а не от корня `dtv_device` — отсюда
`dtv_device/import/kors` + `import/libjam/submodules/cmake-useful`. Второй дефект рядом —
открытие через поиск вверх, из-за которого пустой каталог подмодуля открывался как его
родитель.

**Косвенные улики гипотезы при этом верны**, и теперь их показывают предупреждения:
- `core.ignorecase` не записан ни в одном из 21 конфига — на NTFS это `false`, как после
  клона на Linux;
- worktree `dtv_device_master` зарегистрирован по пути, которого здесь нет (`prunable`).
  На машине тестирования он был `E:\home\user\work\dtv_device_master` — то есть
  `/home/user/work/…` с Linux; такой путь проверка помечает как путь другой ОС и
  предлагает `git worktree repair`.

Если на машине тестирования `.git`-файлы подмодулей действительно содержат абсолютные
Linux-пути, после этой сборки это будет видно сразу: и в ошибке открытия, и в
предупреждении — с командой `git submodule absorbgitdirs`.

## 3. Проверки состояния

**Реализованы (обязательные):**

| Проверка | Почему | Как |
|---|---|---|
| `core.ignoreCase` ≠ ФС | переименование только регистром ведёт себя неверно; ровно это SmartGit показал для `import/libjam` | проба: файл в `.git` и обращение в другом регистре; ключа нет = `false` |
| Висячий путь подмодуля | подмодуль не откроется, а сообщение git ничего не объясняет | `.git`-файл → `gitdir:` → существует ли; абсолютный путь чужой ОС помечается |
| Висячий путь worktree | `git worktree` падает на таких записях, а путь с Linux на Windows выглядит как `E:\home\…` | `.git/worktrees/*/gitdir` → существует ли |

Проверка worktree заодно покрывает кандидата «obsolete worktrees»: папка удалена → «Pruning
forgets it».

**Предложены, не реализованы — нужно ваше решение:**

| Кандидат | За | Против |
|---|---|---|
| Неинициализированные submodules | новичок не понимает, почему в папке пусто | дерево уже помечает их `not initialised` и по двойному клику предлагает Initialize; второе место — шум |
| `core.longpaths` выключен на Windows при длинных путях | checkout падает с `Filename too long` | чтобы узнать «есть ли длинные пути», надо пройти индекс; на большом репозитории это не бесплатно — разумно только при первом открытии и с кешем |
| Слишком старая версия git | часть команд (`worktree repair` ≥ 2.29, `--show-origin` ≥ 2.8) отсутствует | версия уже видна в About; порог надо выбрать осознанно |

Моя рекомендация — `core.longpaths` (реальная поломка без очевидной причины) и старая
версия git с порогом 2.30; неинициализированные подмодули оставить дереву.

## 4. Проверено

- `git_engine`: `gitlink` 7, `health` 10, `config_file` 14, `submodules` 22, `version` 2;
- `app_state`: `repositories` 24, включая сценарий «второй подмодуль после первого»;
- `cogit`: 69, включая вложенные меню в таблице клавиш;
- фронтенд: `health` 11, `exit` 7, `suppressions` 7, `config-syntax` 8, `module-tree` 32,
  `refs` store 13, `design-tokens` 1;
- на живом `dtv_device`: обход проверок — 72 мс на 21 репозиторий; положение подмодулей
  совпало с `git rev-list --left-right --count`.

Глазами на машине тестирования не проверено.
