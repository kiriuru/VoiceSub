# Стратегия изоляции Remote Mode

Эта ветка вводит LAN-only remote mode, не ломая существующий local-first workflow.

## Решения

1. Local mode остаётся default.
2. Remote mode — opt-in и по умолчанию выключен.
3. Не добавляются cloud/SaaS/accounts/auth.
4. Ответственность Controller и Worker разделена.
5. Поведение локального API/UI/overlay не меняется, когда remote mode выключен.
6. Desktop launcher показывает remote роли только как явные вторичные профили, а не как default путь запуска.

## Важные рабочие папки (для экспериментов/проверок)

- `SST desktop remote SST/`: тестовая рабочая папка для экспериментов с remote mode.
- `desktop remote clean/`: «чистая» папка для publish validation и проверок clean-start.

## Правила безопасной реализации

1. Добавлять remote config только за явными флагами.
2. Не менять существующие default значения локального старта.
3. Иметь отдельные runtime entrypoints для Controller и Worker.
4. Subtitle routing остаётся на стороне Controller.
5. ASR+translation в remote режиме исполняется на стороне Worker.
6. Для нестабильного LAN иметь reconnect и heartbeat.
7. Controller bootstrap остаётся лёгким, worker bootstrap — только local AI.

## Критерии готовности remote mode

1. Local mode работает без remote setup.
2. Remote mode работает в пределах одного LAN при явном включении.
3. Overlay и export остаются на стороне Controller.
4. Worker может выполнять AI-only pipeline из входящего remote audio.
