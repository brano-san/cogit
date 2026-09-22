# Итерация: замечания к графу после первого просмотра

| № | Замечание | Причина | Что сделано |
|---|---|---|---|
| 1 | Верхняя точка — последний коммит `master`, а не текущей ветки | Порядок по дате без исключений | HEAD выходит первым, как только вышли его потомки (R-164) |
| 2 | Ветка наезжает на коммит | Полоса, сдвинутая новой вершиной ветки, поворачивала на всю строку и проходила в 0,5 px от кольца | Сдвиги такой строки поворачивают в верхней половине, 3,7 px от кольца; остановившаяся полоса отдаёт колонку строкой ниже (R-163) |
| 3 | Аватар слева от имени | — | Аватар после имени в списке коммитов (F-266) |

Попутно найдено проверкой на случайных историях: стрелка скрытого второго родителя лежала на
линии первого. Теперь она наклонена вправо-вниз.

Не менялось: панель деталей коммита — там аватар по-прежнему перед именем, замечание было про
список.

## Тесты

- `graph_engine`: `a_lane_a_tip_pushes_aside_turns_before_the_ring`,
  `a_lane_that_ends_gives_its_column_back_on_the_next_row`,
  `the_arrow_of_a_hidden_parent_has_its_column_to_itself`,
  `a_hidden_second_parent_points_away_from_the_line_that_goes_on`,
  `a_lane_leaving_the_column_of_the_next_node_turns_before_its_ring`,
  `no_line_but_its_own_reaches_a_ring_on_random_histories`;
  `a_tip_is_placed_beside_the_lane_it_will_join` больше не проверяет форму сдвига — это
  делает первый тест списка.
- `git_engine`: три теста `the_commit_asked_for_first_…` в `topo.rs`;
  `the_commit_head_is_on_comes_first_even_when_another_branch_is_newer`,
  `a_head_behind_another_branch_still_comes_after_the_commits_on_top_of_it`;
  `lines_that_lived_at_the_same_time_are_read_in_date_order` теперь стоит на самой новой
  ветке — иначе первой строкой был бы HEAD.
- фронтенд: наклонная стрелка; зазор поворота в верхней половине больше 3 px, поворота на
  всю строку — меньше 1 px.

## Второй просмотр

| № | Замечание | Причина | Что сделано |
|---|---|---|---|
| 1 | Два вида поворотов | Линии узла поворачивали за полстроки, проходящие полосы — за всю | Все повороты — полстроки, через колонку полосы на высоте кольца (R-165) |
| 2 | При быстрой прокрутке кольца без текста | Строки прокручивал браузер в своём потоке, холст — код | Строки в закреплённом слое вместе с холстом, двигаются кодом (R-166) |

Тесты: `every_turn_takes_half_a_row_on_random_histories`,
`a_lane_a_merge_pushes_aside_turns_alongside_the_new_line`; общий генератор
`random_layouts` для двух проверок на случайных историях. Переписаны ожидания
`a_lane_ending_in_the_middle_slides_…` (сдвиг — `Top` 2→1 и `Bottom` 1→1 вместо
`Through` 2→1) и `an_octopus_opens_its_new_lanes_…` (сдвиг — `Bottom` 1→3). Прокрутку
тестом не закрепить — это разметка; проверяется глазами (F-267).
