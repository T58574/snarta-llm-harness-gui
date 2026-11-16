# Бенчмарки Snarta

В папке `bench/` лежат три набора инструментов на стандартной библиотеке Python 3 (внешние пакеты не обязательны):

1. `bench_ncmoe.py` подбирает оптимальный флаг `-ncmoe` для MoE-моделей вроде Gemma 4 26B-A4B.
2. `bench_mmlu.py` проверяет качество ответов на датасетах English MMLU, русском RuMMLU, стресс-наборе Hardcore и пользовательских тестах.
3. `bench_tui.py` дает интерактивное терминальное меню поверх этих скриптов.

## Подбор `-ncmoe` (`bench_ncmoe.py`)

Скрипт по очереди запускает `llama-server` с разными значениями `-ncmoe`, прогревает модель, отправляет контрольные запросы к `/v1/chat/completions` и строит сравнительную таблицу с лучшим ускорением относительно базового прогона.

### Быстрый запуск

```powershell
python bench/bench_ncmoe.py
```

Если пути не переданы, скрипт ищет `llama-server.exe` в `runtime/` и первую модель `.gguf` в `models/`.

### Примеры

Прогон диапазона `2..26` с шагом `2`:

```powershell
python bench/bench_ncmoe.py `
  -s "target\release\runtime\b1370-rocm-gfx110X-x64\llama-server.exe" `
  -m "target\release\models\gemma-4-26B-A4B-it-ultra-uncensored-heretic-Q8_0.gguf" `
  --range 2 26 2 `
  --cooldown 4 `
  --runs 2 `
  --max-tokens 256
```

Точечный список значений:

```powershell
python bench/bench_ncmoe.py --ncmoe "none,4,12,18,24" --cooldown 5
```

### Частые параметры `bench_ncmoe.py`

- `-s, --server`: путь к `llama-server.exe`.
- `-m, --model`: путь к модели `.gguf`.
- `--range START STOP [STEP]`: диапазон значений `-ncmoe`.
- `--ncmoe`: список значений через запятую (например, `"none,2,4,6,8,12,18,24"`).
- `--mtp [FILE|auto|none]`: драфт-слой MTP (`-md <file> --spec-type draft-mtp`).
- `--no-baseline`: пропустить базовый запуск без `-ncmoe`.
- `--cooldown`: пауза в секундах между запусками для очистки VRAM (по умолчанию `4.0`).
- `--pre-unload-delay`: пауза перед остановкой сервера (по умолчанию `1.0`).
- `-r, --runs`: число контрольных прогонов (по умолчанию `2`).
- `--warmup`: число прогревочных прогонов (по умолчанию `1`).
- `--max-tokens`: лимит токенов генерации (по умолчанию `256`).
- `-c, --ctx`, `-ngl`, `-b, --batch`, `-ub, --ubatch`, `--kv-type`, `--load-mode`, `--fa`, `--reasoning`, `--reasoning-effort`: параметры движка.
- `--host`, `--port`, `--prompt`, `--extra-args`, `-o, --output-dir`, `-v, --verbose`: адрес сервера, свой запрос и каталог отчетов (`bench/reports` по умолчанию).

По итогам работы скрипт сохраняет отчет `.md` и сырые данные `.json` в `bench/reports`. Лучший флаг затем можно вписать в `snarta.toml`:

```toml
[model_defaults]
extra_args = "-ncmoe 24"
```

## Оценка качества MMLU, RuMMLU и Hardcore (`bench_mmlu.py`)

`bench_mmlu.py` проверяет выбор ответа (`A`, `B`, `C`, `D` и кириллические варианты) и строит графики точности против скорости в автономном HTML-дашборде.

### Встроенные наборы вопросов

- **English MMLU** (`bench/data/mmlu_sample.json`): сбалансированная выборка на 100 вопросов по четырем категориям (`STEM`, `Humanities`, `Social Sciences`, `Other`). Флаг `--download-official` загружает полный архив Hendrycks на 57 предметов.
- **RuMMLU / MERA** (`--dataset rummlu`): 120 вопросов на русском языке по направлениям `STEM`, `Humanities`, `Social Sciences`, `Russian Language & Culture` и `IT Edge Cases`.
- **Hardcore Stress** (`--dataset hardcore`): 100 сложных задач с логическими ловушками и многошаговой дедукцией для проверки деградации младших квантов (`Q3_K_L`, `Q4_K_M`, `Q5_K_M`, `Q8_0`).

### Примеры запуска `bench_mmlu.py`

```powershell
# Базовая проверка на 100 вопросах
python bench/bench_mmlu.py

# Сравнение нескольких квантов с автоподбором -ncmoe
python bench/bench_mmlu.py `
  --models "models\*Q4*.gguf,models\*Q5*.gguf,models\*Q6*.gguf,models\*Q8*.gguf" `
  --ncmoe auto `
  --cooldown 5

# Проверка уже запущенного llama-server
python bench/bench_mmlu.py --server-url "http://127.0.0.1:8089" --model-name "gemma-4-Q5_K_M"

# Русский датасет с фильтром категорий
python bench/bench_mmlu.py --dataset rummlu --categories "Russian Language & Culture,Humanities"

# Стресс-тест на младших квантах
python bench/bench_mmlu.py --dataset hardcore --models "models\*Q3*.gguf,models\*Q4*.gguf,models\*Q5*.gguf" --ncmoe auto --cooldown 5
```

### Основные параметры `bench_mmlu.py`

- `-s, --server`, `-m, --model`, `--models`, `--models-dir`: выбор бинарника и моделей (через список или glob-маску).
- `--server-url`: подключение к работающему серверу без перезапуска.
- `--dataset`, `--download-official`: выбор файла тестов или загрузка полного MMLU.
- `--categories`, `--subjects`, `--limit`, `--limit-per-subject`: фильтрация предметов и объема выборки.
- `--ncmoe`: число или `auto` (подбор по кванту: Q4 = 7, Q5 = 10, Q6 = 14, Q8 = 17).
- `--cooldown`, `--no-plots`: пауза между моделями и отключение HTML/PNG-графиков.

В папке `bench/reports` создаются файлы `mmlu_<имя>_<дата>.md`, `.json`, `_dashboard.html` и `_chart.png` (если установлен `matplotlib`).

## Интерактивный TUI-раннер (`bench_tui.py`)

`bench_tui.py` объединяет все режимы в одном консольном интерфейсе для Windows Terminal, PowerShell и Linux:

```powershell
python bench/bench_tui.py
python bench/bench_tui.py --dataset snarta_it_test_chatgpt
python bench/bench_tui.py --mock
```

Внутри меню клавиши `Up/Down` переключают пункты, `Space` и `a` выбирают модели, `Left/Right` листают датасеты из `bench/data/`, а клавиша `O` на итоговом экране открывает HTML-дашборд.

## Пользовательские датасеты

Поместите файл `.json`, `.jsonl` или `.csv` в `bench/data/`, и он автоматически появится в `bench_tui.py` и `--dataset <имя>`:

- В JSON допустим массив объектов или словарь с ключом `questions`, `data` или `tests`. У каждого элемента нужны поля `question`, `choices` (список или словарь `A..D`) и `answer` (`A..D`, `А..Г`, индекс `0..3` или точный текст ответа). Поля `subject` и `category` необязательны.
- В CSV нужны колонки `question, a, b, c, d, answer`.
