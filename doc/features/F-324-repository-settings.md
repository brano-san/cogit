# F-324 · Repository ▸ Settings…

Диалог настроек репозитория с вкладками User, Fetch and Pull, Push, Signing, Encoding,
Tag-Grouping: у каждой опции видно, задана ли она в репозитории или наследуется (и какое
значение), и где хранится — всё пишется в `.git/config` репозитория. Проверить: на вкладке
User ввести имя, Save — `git config --local user.name` показывает его; снова открыть,
«Use inherited», Save — ключ исчез, под полем «inherited: <глобальное имя>».
