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

## Документированный порядок запуска

Dashboard Help теперь фиксирует операторский порядок для remote mode:

1. Сначала запускается Worker (`Remote Worker` profile или `start-remote-worker.bat`).
2. Затем запускается Controller (`Remote Controller` profile или `start-remote-controller.bat`).
3. На Controller задаётся `Worker Base URL`.
4. Перед pairing и runtime start выполняется `Check Worker Health`.
5. Создаётся/проверяется pair, затем обновляется remote state.
6. Выполняется `Sync Worker Settings`, чтобы Worker остался на local AI path.
7. Выполняется `Prepare Remote Run`.
8. Запускается и проверяется Worker runtime.
9. Controller/Worker bridge windows остаются открытыми на время remote session.
10. `Start` в основном dashboard запускает microphone capture и remote audio/result flow.
