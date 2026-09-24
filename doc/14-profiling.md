# Профилирование: как читать лог рабочего дня

Цель: пользователь работает день, забирает `cogit.log`, отдаёт его — и по нему видно,
какое действие тормозило и чья это вина. Ответ должен различать «сеть», «удалённый сервер»,
«много файлов у нас на диске» и «наш код».

## 1. Где лог

`Tools → Reveal Log File` или команда палитры «Reveal Log File» открывает папку с файлом.
Путь также печатает `Help → About Cogit`. Файл `cogit.log`, ротация по 10 МБ, два архива.

## 2. Отдельный поток

Всё профилирование пишется в target `cogit::profile` на уровне `info`, и этот уровень
**не зависит** от настройки `Log level`: иначе пользователь, поставивший `warn`, привёз бы
пустой лог. Выборка:

```sh
grep 'cogit::profile' cogit.log
```

## 3. Три вида записей

| `kind` | Откуда | Что значит |
|---|---|---|
| `ipc` | `src-tauri/src/commands/mod.rs`, хелпер `blocking` | одна строка на каждый вызов команды: `op`, `ms`, `ok` |
| `ui` | вебвью, команда `report_timing` | интервал между действием пользователя и отрисовкой результата |
| `network` | `fetch`/`pull`/`push` | разбор по фазам `git --progress` |
| `mem` | вебвью, команда `report_memory` | что рендерер держит: heap, DOM, подписки, размеры кэшей |
| `procmem` | хост, `src-tauri/src/webview_memory.rs` | рабочее множество процессов `msedgewebview2.exe` |

Порог: `ipc` молчит быстрее 5 мс, `ui` — быстрее 120 мс. Иначе прокрутка списка забьёт файл.

## 4. Вычитание: где именно потеря

`ui` меряет то, что видит человек, `ipc` — то, что делал бэкенд. Разница между ними — это
сериализация IPC и отрисовка вебвью, то есть наша вина на фронтенде.

```
kind=ui  op=select-commit ms=480 detail="14 files"
kind=ipc op=commit_details ms=3 ok=true
kind=ipc op=commit_files   ms=6 ok=true
```

480 − 9 = 471 мс во фронтенде. Если бы `commit_files` показал 400 мс — виноват бэкенд.

## 5. Фазы сети

`git fetch|pull|push --progress` печатает свои этапы в stderr. `crates/git_engine/src/phases.rs`
разбирает их и складывает время по фазам; `phase_of` знает восемь:

| Фаза | Строка git | Чья задержка |
|---|---|---|
| `negotiating` | ничего не печатается | соединение, DNS, аутентификация |
| `enumerating`, `counting`, `compressing` | `remote: …` | удалённый сервер |
| `receiving`, `writing` | `Receiving objects`, `Writing objects` | сеть |
| `resolving` | `Resolving deltas` | наш диск и CPU |
| `updating-tree` | `Updating files` | наш диск, много файлов |

Строка выглядит так:

```
kind=network op=pull remote=origin ms=8420 objects=2000 bytes=3670016
  kib_per_s=512.3 phases="negotiating=180 receiving=7900 resolving=340"
  slowest="network transfer"
```

`slowest` — это фаза с наибольшим временем, переведённая в ответ на вопрос «кто виноват».
`kib_per_s` отвечает отдельно: 512 КиБ/с при гигабитном канале — проблема не в объёме.

## 6. Что добавлять дальше

Новая долгая операция получает замер в том же формате. На бэкенде — ничего делать не надо,
хелпер `blocking` уже оборачивает каждую команду. На фронтенде — `measurer()` из
`$lib/timing`, `watch.stop(detail)` после того, как состояние обновилось.

## 7. Память рендерера

Рендерер WebView2 умирает с `Out of Memory` молча: консоль исчезает вместе с ним. Поэтому
обе серии пишутся в файл и с одинаковым шагом в 10 секунд, чтобы ложиться рядом по времени.

```sh
grep 'kind="mem"' cogit.log
grep 'kind="procmem"' cogit.log
```

`kind=mem` пишет сам вебвью и **только в отладочной сборке**: замер стоит обхода DOM, а в
релизе эти строки некому читать. `kind=procmem` пишет хост — он переживает смерть рендерера,
так что последние цифры перед падением в логе остаются. Считаются только процессы-потомки
нашего pid: WebView2 запущен и у других приложений.

```
kind=mem used_kib=54358 total_kib=56372 limit_kib=4292608 dom=351 listeners=7
  caches=avatarRows=0 blameLines=0 commitFiles=0 diffHunks=0 graphEdges=99999
  graphRows=100000 openRepos=1 outputEntries=0 overlapRows=0
kind=procmem processes=6 rss_kib=493208 largest_kib=180916
```

Размера heap мало, чтобы отличить утечку от большого репозитория, поэтому рядом идут
счётчики. Они и разделяют диагнозы:

| Что растёт | Диагноз |
|---|---|
| `used_kib` при неизменных `dom`, `listeners` и `caches` | фрагментация или мусор вне JS |
| `caches` | стор копит и не вытесняет |
| `listeners` | подписка без `unlisten` |
| `dom` при виртуализированном списке | узлы остаются отцеплёнными |
| `rss_kib` без роста `used_kib` | память вне JS-heap: текстуры, blob-ы, декодированные строки |
| число вызовов одной команды `kind=ipc` | цикл эффектов ([R-93](12-risks.md)) |

Последняя строка — то, чем R-93 и был найден: heap выглядел безобидно, а в логе лежало
26 814 вызовов `open_repository` за четыре минуты.

`limit_kib` — потолок JS-heap, который назначил себе Chromium (около 4 ГиБ). Падение может
прийти задолго до него: в R-93 рендерер умер на 14 ГиБ рабочего множества, из которых на
JS-heap приходилась малая часть.

Нагрузочный прогон, которым это ловится, — `scripts/oom/`, инструкция в
[`scripts/oom/README.md`](../scripts/oom/README.md).

## 8. Профиль записи: где время внутри `git`

Снят 24.09.2026 через `GIT_TRACE2_PERF`, git 2.51.0.windows.1, набор dirty бенчмарка
(`node scripts/bench/fixtures.mjs --root <папка> --only dirty,small`), ядра 16–31, медиана
из 7 запусков через `spawnSync` (так же, как запускает Rust).

| Что | мс | Из чего |
|---|---|---|
| `git version` | 41 | запуск процесса на Windows — столько стоит любой вызов |
| `git add --all`, 2 003 пути через stdin, литерально | 368 | до `read_directory` 51 мс (запуск 31, preload индекса 9, обход 11); 290 мс — хэширование и запись 2 000 loose-объектов; запись индекса < 1 мс |
| то же без списка путей (`git add --all`) | 328 | −40 мс: сверка каждой записи с каждым pathspec (R-311) |
| то же с `-c core.bigFileThreshold=1` | 177 | блобы идут потоком в один pack вместо 2 000 файлов в `.git/objects` (R-312) |
| `git hash-object --stdin-paths` (без записи) | ≈140 | чтение и хэширование тех же файлов |
| `git commit -m`, small | ≈96 | 52 мс своей работы, 44 мс — дочерний `git maintenance run --auto --detach`, которого родитель ждёт (R-314) |
| `git rev-parse HEAD` после коммита | 41 | один запуск процесса (R-313) |
| `git stash push --include-untracked`, small | ≈265 | четыре дочерних процесса по ≈50 мс: `update-index` ×2, `clean`, `reset --hard` (R-315) |
| `git stash push` без `-u` | ≈160 | два дочерних: `update-index`, `reset --hard` |

Не дали ничего (в пределах шума ±15 мс): `-c index.threads=true` (индекс 2 003 записей
читается за 0,2 мс, и `true` — уже умолчание), `-c core.fsmonitor=false`,
`-c core.preloadIndex`, `-c core.fscache=false`, `core.fsync=none` и `core.fsyncMethod=batch`
(loose-объекты по умолчанию не синхронизируются), `core.compression=0`.
`core.untrackedCache` не пробовался как флаг: он пишет расширение `UNTR` в индекс, а формат
индекса пользователя не меняется.

Хуков в замерах нет; с хуком `commit` дольше ровно на его время — git ждёт его до конца.
