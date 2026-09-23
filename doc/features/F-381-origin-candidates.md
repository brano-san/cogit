# Происхождение строки и Go Deeper

Клик по строке в Blame запускает фоновый поиск происхождения её блока: карточка у строки
пишет `Appeared here — Lines first appeared at this position` (или откуда блок перенесён
или скопирован) с оценкой вида `single origin, high likelihood`, Blame+Origins показывает
всех кандидатов, а `Go Deeper` (`Ctrl+D`, полоса `Blame (go deeper)` справа) открывает
версию до коммита-источника на той же строке и ищет снова.
