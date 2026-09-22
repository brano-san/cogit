# Итерация: граф коммитов по образцу SmartGit

Только графическая часть: раскладка полос, форма линий, узлы, цвета, левый край текста.
Даты, авторы, аватары, хеши и метки ссылок не менялись. Решения — R-161 (раскладка и
отрисовка) и R-162 (порядок коммитов).

## Отчёт по требованиям

| Требование | Что сделано |
|---|---|
| Главная линия слева, прямая, светлая, чуть толще | Колонка 0 за первичным ref; `--graph-main` = цвет текста, 2.75 px против 2 |
| Остальные линии — один серый | `--graph-line` = `--text-secondary`, в обеих темах |
| Ширина по строке, текст сразу за последней линией | `GraphRow.width`, `padding-left = textX(width)` у каждой строки |
| S-диагонали в одну строку | Кубическая кривая, вертикальные касательные, без горизонтальных участков |
| Уплотнение влево | Закончившаяся полоса удаляется из списка в той же строке |
| Новые полосы справа от узла | Второй и следующие родители — сразу за колонкой узла, в порядке родителей |
| Одинаковые полые кольца | Заливка фоном панели, на выделенной строке — ещё и фоном выделения |
| Стрелка к коммиту вне списка | `Segment.arrow`; работает и в отфильтрованном списке (F-264) |
| Алгоритм, шаги 1–8 | [07-graph-rendering.md §3](../07-graph-rendering.md#3-алгоритм-раскладки) |
| Геометрия | [07-graph-rendering.md §5](../07-graph-rendering.md#5-отрисовка-на-canvas), шаг 16 px |
| Предел ширины | 40 колонок, дальше обрезка с затуханием |
| Цвета по полосам | Настройка `Colored branch lines`, по умолчанию выключена (F-263) |
| Розовая вертикаль | См. ниже |
| Бюджеты скорости | Выполнены, см. ниже |
| 7 модульных тестов | Первые семь в `crates/graph_engine/tests/lanes.rs` |

**Отступления.** Вершина ветки, которую никто не ждал, встаёт не в конец списка, а рядом с
полосой, уже ждущей её первого родителя, — иначе линия шла бы к ней через весь граф.
Первичный ref выбирается только из отмеченных в Branches. Порядок коммитов сменён с обхода в
глубину на `--date-order` (R-162): при той же раскладке он даёт граф на треть уже.

## Розовая вертикаль

Причина — порядок, а не отрисовка: при обходе в глубину коммит мог выйти на тысячи строк
раньше ветки, которая к нему приходит, и полоса, ждущая его, тянулась через весь экран
своим цветом полосы. На vega (11 053 коммита) в новом порядке нет ни одного ребра «родитель
выше потомка», и главная линия во всех строках в колонке 0. Старую картинку без старого кода
не воспроизвести, поэтому это объяснение по механизму.

## Ширина графа на vega

| | p50 | p90 | p99 | max |
|---|---|---|---|---|
| Новая раскладка, обход в глубину | 12 | 27 | — | 48 |
| Новая раскладка, `--date-order` | 10 | 18 | 27 | 36 |
| То же, только HEAD | 9 | 16 | — | 32 |

## Скорость, 50 000 коммитов

`cargo test -p app_state --test performance fifty_thousand`, три прогона на простаивающей
машине; «до» — коммит `f37c57d` во временном worktree.

| Профиль | Замер | До | После | Бюджет |
|---|---|---|---|---|
| release | первые 200 строк | 11 мс | 8 мс | 300 мс |
| release | весь граф | 316–324 мс | 204–210 мс | 500 мс |
| отладочный | первые 200 строк | 14 мс | 11–12 мс | 300 мс |
| отладочный | весь граф | 467 мс | 356–368 мс | 500 мс |

Быстрее не раскладка, а чтение: обход читал каждый коммит дважды. Кэш объектов `gix` на
4 МБ убрал второе чтение. Первая версия раскладки без него была медленнее старой (504 мс
в отладочном профиле, выделение памяти на каждую строку) — буферы курсора
переиспользуются.

## Тесты, закреплявшие отменённые правила

### Индекс полосы не меняется, свободная колонка переиспользуется

| Было | Стало |
|---|---|
| `free_lane_is_reused_before_widening_the_graph` | `a_lane_ending_in_the_middle_slides_the_lanes_right_of_it_left_in_that_row` |
| `a_lane_is_reused_once_its_branch_has_ended` | `a_finished_branch_gives_its_column_back` — уплотнение вместо переиспользования |
| `a_full_row_of_lanes_appends_a_new_one` | `an_octopus_opens_its_new_lanes_right_of_the_node_in_parent_order` — справа от узла, а не в конце |
| `branches_off_the_mainline_go_to_its_right` | `a_branch_merged_back_has_two_lanes_while_it_lives_and_the_main_one_never_moves`, `a_tip_is_placed_beside_the_lane_it_will_join` |
| `the_first_parent_keeps_the_lane_of_its_child` | `a_linear_history_is_one_lane_of_vertical_segments` и инвариант `the_main_line_holds_column_zero_on_random_histories` — полоса та же, колонка может смещаться |
| `snapshot_diamond`, `snapshot_nested_branches`, `snapshot_octopus`, `snapshot_two_roots` (`insta`) | Утверждения о колонках и сегментах: тесты выше и `independent_roots_are_never_joined`; `insta` из `graph_engine` убран |

### Слияние — кольцо, переходы — колена

| Было | Стало |
|---|---|
| `draws the ring of a merge thick enough to read` | `draws a ring big enough to aim at with a mouse` — одно кольцо для всех |
| `keeps the ring inside the row, so two merges do not touch` | `leaves a gap between a ring and the one below it` |
| `draws a dot big enough to aim at with a mouse`, `leaves a gap between a dot and the one beside it`/`below it` | То же про кольцо: `…a ring…` |
| `rounds the corner by less than half a lane, or the elbow overshoots` | `leaves and arrives vertically`, `meets the node at its centre`, `ends a row where the next one starts, so a lane runs on without a gap` |
| `uses an even line width, so nothing has to be nudged half a pixel` | `draws the main line a little thicker than the rest, as SmartGit does` — сдвига нет, потому что у линии и кольца один центр |
| `a_commit_with_two_parents_is_marked_as_a_merge`, `a_root_commit_is_marked_as_such` | `a_root_and_a_merge_are_told_apart` — тип узла остался в данных, рисуется одинаково |

### Желоб графа общей ширины

| Было | Стало |
|---|---|
| `grows with the widest lane`, `widens the gutter to match` | `moves one column right for every column the row uses`, `moves the text to match`, `starts right of the one column a linear history uses`, `treats a row with no columns as one` |
| `never eats more than a quarter of the panel` | `stops at the column cap, so a pathological row cannot push the text away` |
| `indexes every edge once, not once per frame`, `keeps the per-frame edge lookup independent of history size` | Удалены: рёбер между строками больше нет, сегменты лежат в своей строке |

### Отфильтрованный список — плоский (R-51)

| Было | Стало |
|---|---|
| `a_filtered_result_is_a_flat_list_without_edges` | `a_filtered_result_links_what_it_shows_and_ends_the_rest_in_an_arrow` |
| `a_search_inside_a_narrowed_graph_is_still_flat` | `a_search_every_commit_matches_draws_the_whole_graph` |
| `a_merge_keeps_its_two_lanes_within_one_chunk` | `a_diamond_is_two_columns_wide_while_it_is_open` |
| — | `the_main_column_follows_head_when_head_is_ticked_and_nothing_when_it_is_not` |

### Обход в глубину (R-140)

| Было | Стало |
|---|---|
| `a_topological_walk_keeps_each_line_of_history_together` | `lines_that_lived_at_the_same_time_are_read_in_date_order` — ожидание обратное |
| `two_lines_of_history_stop_interleaving` | `lines_that_lived_at_the_same_time_stay_in_date_order` |
| `an_octopus_merge_keeps_each_of_its_branches_together` | `an_octopus_merge_comes_out_in_date_order` |
| `a_parent_dated_after_its_child_is_still_emitted_once` | `a_parent_dated_after_its_child_still_comes_after_it`, `a_child_committed_with_a_clock_behind_its_parent_still_comes_first` |
| `several_tips_are_started_newest_first` | `several_tips_come_out_newest_first` |
| `a_lookahead_too_small_to_group_still_orders_topologically` | `every_commit_is_emitted_exactly_once` на окнах 1–64: по дате без перекоса окно не важно |
| `both_walks_see_the_same_commits` | Удалён: обход один |

### Правило осталось, тест переименован

`the_mainline_takes_lane_zero_even_when_it_is_not_the_newest_commit` →
`the_main_column_waits_empty_until_the_primary_ref_arrives`;
`lane_zero_is_not_reused_after_the_mainline_ends` →
`column_zero_is_not_given_away_after_the_main_line_ends`;
`the_column_is_still_reserved_after_a_chunk_boundary` →
`the_main_column_stays_reserved_across_a_chunk_boundary`;
`with_no_mainline_named_the_first_commit_still_takes_lane_zero` →
`with_no_primary_ref_the_first_commit_takes_column_zero`;
`linear_history_stays_in_a_single_lane`, `a_linear_history_is_one_column_on_the_left` →
`a_linear_history_is_one_lane_of_vertical_segments`;
`a_diamond_widens_to_two_lanes_and_comes_back` → `a_branch_merged_back_has_two_lanes…`;
`an_octopus_merge_gathers_three_parents` → `an_octopus_opens_its_new_lanes…`;
`rows_continue_counting_across_chunks` →
`a_streamed_history_lays_out_exactly_as_it_would_all_at_once`;
`an_empty_chunk_produces_an_empty_layout` → `an_empty_chunk_lays_out_nothing`;
`without_either_name_the_branch_head_is_on_takes_the_column` — покрыт
`the_branch_head_is_on_takes_the_column_over_master`.

## Проверка

- `cargo nextest run --workspace --exclude cogit` — 1314 passed, 1 skipped;
- `cargo test -p cogit --lib` — 69 passed;
- `npm --prefix frontend run test` — 1046 passed;
- приложение не запускалось: граф глазами рядом со SmartGit не сравнивался.
