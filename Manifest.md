# Cargo AI FDocs — Manifest (v0.1-alpha, updated)

## 1. Обзор проекта

### Проблема
AI-ассистенты (Cursor, Copilot, Claude Code и др.) часто генерируют код по устаревшей документации зависимостей.

### Решение
`cargo-ai-fdocs` синхронизирует документацию библиотек из GitHub в локальную папку проекта, чтобы AI работал с актуальным контекстом, привязанным к версиям из `Cargo.lock`.

### Базовые принципы
- **Mirror lock-file:** документация соответствует версии из `Cargo.lock`.
- **Предсказуемый результат:** повторный запуск при тех же входных данных даёт ту же структуру.
- **Минимальная инвазивность:** не модифицируем файлы проекта вроде `.gitignore`.
- **Умные дефолты:** если `files` не заданы — тянем `README.md` и `CHANGELOG.md`.

---

## 2. Текущие реалии репозитория

На текущем этапе в кодовой базе уже есть модули:
- `config` (чтение TOML-конфига),
- `resolver` (чтение версий из `Cargo.lock`),
- `fetcher` (GitHub API + raw content),
- `processor/changelog` (подрезка CHANGELOG),
- `index` (генерация `_INDEX.md`),
- `status` (проверка актуальности скачанной документации).

Поддерживается сценарий Rust-экосистемы (папка `docs/ai/vendor-docs/rust`).

---

## 3. CLI и команды (alpha)

Текущий рабочий сценарий:

```bash
cargo ai-docs sync
cargo ai-docs sync --force
cargo ai-docs status
```

> Примечание: бинарник проекта называется `cargo-ai-fdocs`, но в alpha-ветке уже используется cargo-subcommand стиль `ai-docs`.

---

## 4. Конфигурация (текущий формат)

Файл: `ai-docs.toml` в корне проекта.

```toml
[settings]
output_dir = "docs/ai/vendor-docs/rust"
max_file_size_kb = 200
prune = true

[crates.axum]
sources = [{ type = "github", repo = "tokio-rs/axum" }]
ai_notes = "Web framework layer"

[crates.sqlx]
sources = [{ type = "github", repo = "launchbadge/sqlx" }]
files = ["README.md", "CHANGELOG.md", "docs/migration-guide.md"]
ai_notes = "Use sqlx::query! where possible"
```

### Семантика полей
- `settings.output_dir` — куда сохранять docs.
- `settings.max_file_size_kb` — лимит размера файла с обрезкой.
- `settings.prune` — удалять устаревшие папки версий.
- `crates.<name>.sources` — источник документации (в alpha: GitHub).
- `crates.<name>.files` — явный список файлов (если не указан, используются дефолтные).
- `crates.<name>.ai_notes` — заметки для AI в индексах.

---

## 5. Алгоритм `sync` (alpha-контракт)

1. Прочитать `ai-docs.toml`.
2. Прочитать `Cargo.lock` и получить `crate -> version`.
3. (Опционально) выполнить pruning.
4. Для каждого crate из конфига:
   - проверить, есть ли версия в lock;
   - проверить кэш (`crate@version` + `.aifd-meta.toml`);
   - определить git ref (тег, иначе fallback на branch);
   - скачать нужные файлы;
   - обработать CHANGELOG;
   - сохранить файлы и метаданные.
5. Перегенерировать `docs/ai/vendor-docs/rust/_INDEX.md`.
6. Показать итог (`synced/cached/skipped/errors`).

---

## 6. Выходная структура

```text
docs/ai/vendor-docs/rust/
├── _INDEX.md
├── axum@0.8.1/
│   ├── .aifd-meta.toml
│   ├── README.md
│   └── CHANGELOG.md
└── sqlx@0.8.2/
    ├── .aifd-meta.toml
    ├── README.md
    └── docs__migration-guide.md
```

---

## 7. Сетевой слой

- Поддержка токена: `GITHUB_TOKEN` / `GH_TOKEN`.
- Без токена — warning про лимиты GitHub API.
- Fallback на default branch, если тег версии не найден.

---

## 8. Roadmap (согласованная траектория)

### v0.1-alpha (текущий фокус)
- sync/status,
- lock-mirroring,
- fallback-логика,
- `_INDEX.md` и crate metadata,
- pruning и file-size limit.

### v0.2
- `init` (генерация стартового конфига),
- `check` (CI-режим),
- параллельная загрузка,
- расширенные метаданные (детекция изменений конфига).

### v1.0
- стабилизация формата,
- экосистемные sibling-проекты (Node/npm и далее),
- docs.rs как дополнительный источник.

---

## 9. Definition of Done для alpha

Считаем alpha готовой, когда:
- проект стабильно собирается,
- `sync` зеркалит lock-версии в выходной папке,
- `status` корректно сигнализирует `synced/outdated/missing`,
- сетевые и файловые ошибки даются понятными сообщениями.
