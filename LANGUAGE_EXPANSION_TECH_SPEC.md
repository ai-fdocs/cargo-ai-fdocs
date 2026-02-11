# AI Fresh Docs — План и техническое задание для других языков

> Статус: draft для поэтапной реализации sibling-версий после Rust (`cargo-ai-fdocs`) и Node/NPM (`npn/`).

## 1. Цель документа

Этот документ задаёт единый **технический контракт** для расширения AI Fresh Docs на другие языки/экосистемы:
- Python,
- Go,
- Java/Kotlin,
- .NET,
- PHP,
- (опционально) Ruby.

Главная идея: независимо от языка, пользователь получает **одинаковую модель команд, статусов и структуры локального docs-хранилища**, а различается только адаптер экосистемы (lockfile/registry/source resolution).

---

## 2. Общий продуктовый контракт (обязательный для всех реализаций)

### 2.1 Командный интерфейс
Каждая языковая версия должна поддерживать:
- `init` — генерация конфигурации из manifest/lock;
- `sync` — загрузка и обновление docs;
- `status` — человекочитаемый статус по пакетам;
- `check` — CI-режим с корректным exit code.

Опциональные расширения (рекомендуемо):
- `status --format json`
- `check --format json`

### 2.2 Единая структура output
Для каждой экосистемы сохраняем docs в:

```text
fdocs/<ecosystem>/
├── _INDEX.md
├── <package>@<version>/
│   ├── .aifd-meta.toml
│   ├── _SUMMARY.md
│   ├── README.md
│   └── ...
└── ...
```

Где `<ecosystem>`: `rust`, `node`, `python`, `go`, `java`, `dotnet`, `php`.

### 2.3 Семантика кеша
- Ключ кеша: `<package>@<version>` + `config_hash`.
- `config_hash` вычисляется от стабильных полей:
  - `repo` / `source`
  - `subpath`
  - `files` (детерминированный порядок)
- `ai_notes` не должен инвалидировать скачивание.

### 2.4 Поведение sync
- Best-effort: сбой по одному пакету не прерывает sync целиком.
- Прозрачная итоговая статистика: `synced/cached/skipped/errors`.
- Поддержка `--force` для игнорирования кеша.

### 2.5 Поведение check
- `exit 0` — всё актуально;
- `exit 1` — есть missing/outdated/config_changed/not_in_lockfile.

---

## 3. Архитектурный шаблон для новых языков

Каждая реализация должна иметь одинаковые логические модули:

1. `config` — чтение/валидация конфигурации;
2. `resolver` — получение `package -> version` из lockfile;
3. `registry` — metadata из language registry;
4. `fetcher` — загрузка файлов документации;
5. `storage` — сохранение файлов/метаданных и cache check;
6. `summary/index` — `_SUMMARY.md` и `_INDEX.md`;
7. `commands/*` — `init/sync/status/check`;
8. `cli` — wiring команд и ошибок.

Это необходимо для унификации поддержки и упрощения последующего переноса в отдельные репозитории.

---

## 4. Источники документации: единая политика

### 4.1 Рекомендуемая стратегия
- **Primary source:** VCS-репозиторий (обычно GitHub), если он явно и надежно определён.
- **Fallback/experimental:** package tarball/архив из registry.

### 4.2 Почему так
- VCS чаще содержит полную документацию и историю;
- Tarball точнее отражает опубликованный артефакт;
- в разных экосистемах coverage разный — стратегия должна быть переключаемой.

### 4.3 Требование к реализации
В конфиге должен быть флаг для смены source strategy (или явный `source_mode`) без ломки CLI-контракта.

---

## 5. Приоритетная матрица языков

## 5.1 Python (приоритет P1)

### Источники и входные файлы
- Manifest: `pyproject.toml`, `requirements.txt` (fallback).
- Lock: `poetry.lock`, `uv.lock`, `requirements.txt` + pinned versions.
- Registry: PyPI JSON (`/pypi/<name>/json`).

### Особенности
- Нормализация имён (`-` vs `_`, case-insensitive).
- Множественные источники зависимостей (poetry/pip/uv/pdm).
- У некоторых пакетов слабые ссылки на repo.

### MVP scope
- Поддержать `poetry.lock` + `requirements.txt` pinned.
- Извлекать `project_urls`/`home_page`.
- Скачивать docs из GitHub, fallback на sdist tarball.

### Риски
- Неполные/грязные metadata в PyPI.
- Разные lockfile-форматы.

---

## 5.2 Go (приоритет P1/P2)

### Источники
- Manifest: `go.mod`.
- Lock-ish: `go.sum`.
- Source resolution: module path (часто сразу VCS URL).

### Особенности
- Не все модули GitHub-only.
- Версии псевдотеги (`v0.0.0-...`) требуют аккуратного ref mapping.

### MVP scope
- Поддержать модульные зависимости верхнего уровня.
- GitHub-first fetch по module path.
- Корректная обработка псевдоверсий.

### Риски
- Сложные корпоративные прокси/private modules.

---

## 5.3 PHP / Composer (приоритет P2)

### Источники
- Manifest: `composer.json`.
- Lock: `composer.lock`.
- Registry: Packagist API.

### Особенности
- Хорошая предсказуемость metadata.
- Часто качественные ссылки на GitHub.

### MVP scope
- Parse `composer.lock`.
- Repo extraction из Packagist metadata.
- Sync default docs + explicit files.

### Риски
- Разные схемы source/dist в lock.

---

## 5.4 .NET / NuGet (приоритет P2)

### Источники
- Manifest: `*.csproj`.
- Lock: `packages.lock.json`.
- Registry: NuGet API (`registration`, metadata).

### Особенности
- Multi-target frameworks.
- В metadata может быть `repositoryUrl`, но не всегда.

### MVP scope
- `packages.lock.json` parser.
- Repo extraction + GitHub sync.
- Базовый status/check для CI.

### Риски
- Сложные enterprise-репозитории и приватные feeds.

---

## 5.5 Java/Kotlin (Maven/Gradle) (приоритет P3)

### Источники
- Manifest: `pom.xml`, `build.gradle*`.
- Lock/dependency graph: `gradle.lockfile` или resolved dependency tree.
- Registry: Maven Central metadata + POM SCM section.

### Особенности
- Наиболее сложный dependency resolution.
- Важна корректная поддержка group/artifact/version.

### MVP scope
- Фокус на lockfile-first варианте (Gradle lockfile).
- SCM extraction из POM.
- GitHub sync + fallback.

### Риски
- Высокая вариативность сборки.

---

## 6. Единый конфиг-контракт для multi-language

Рекомендуется унифицированная схема (пример):

```toml
[settings]
output_dir = "fdocs/python"
max_file_size_kb = 200
prune = true
sync_concurrency = 8
source_mode = "github_primary" # github_primary | registry_archive_primary

[packages.requests]
repo = "psf/requests"
files = ["README.md", "HISTORY.md"]
ai_notes = "HTTP client best practices"
```

Требования:
- Для всех языков ключ секции: `[packages.<name>]` (не `crates`, не `modules`).
- Поддержка legacy alias допустима, но canonical-формат единый.

---

## 7. Требования к качеству и безопасности

### 7.1 Надежность
- Retry/backoff с jitter для network-запросов.
- Классификация ошибок: `auth`, `rate_limit`, `not_found`, `network`, `parse`.

### 7.2 Безопасность
- Жёсткая валидация путей при распаковке архивов (no absolute paths, no `..`).
- Ограничение размера скачиваемых файлов.
- Защита от некорректного контента/encoding.

### 7.3 Детерминизм
- Стабильная сортировка пакетов в `_INDEX.md`.
- Идемпотентность repeated `sync`.

---

### 7.4 Donor outage / offline tolerance (обязательно)
- "Donors" = внешние источники docs (VCS API/raw, registry API, package archives).
- При недоступности donors реализация обязана:
  - работать в degraded-режиме без краша платформы;
  - не трогать исходный код проекта;
  - не терять ранее валидный кеш;
  - продолжать sync по доступным пакетам;
  - возвращать прозрачный статус в `status/check` и CI.
- Сетевые проблемы не должны превращаться в отказ основного продукта пользователя; это только состояние свежести docs.

## 8. CI/CD контракт для sibling-реализаций

Для каждой языковой папки/репозитория:
- `install`;
- `build`;
- `test`;
- `check` в fixture-проекте.

Минимум:
- PR pipeline: build + tests + check;
- Scheduled/manual pipeline: sync + optional auto-commit обновленных docs.

---

## 9. Пошаговый план реализации (roadmap)

## Этап 1: Python adapter (MVP)
1. Создать `python/` sibling-папку с CLI каркасом.
2. Реализовать `resolver` для `poetry.lock` + pinned requirements.
3. Реализовать `init/sync/status/check` по унифицированному контракту.
4. Добавить CI workflow.

## Этап 2: Composer adapter
1. `php/` sibling-папка.
2. `composer.lock` resolver.
3. Базовый fetch/cache/status/check.

## Этап 3: Go adapter
1. `go/` sibling-папка.
2. `go.mod/go.sum` resolver (MVP по top-level deps).
3. Обработка псевдоверсий.

## Этап 4: .NET adapter
1. `dotnet/` sibling-папка.
2. `packages.lock.json` resolver.
3. NuGet metadata adapter.

## Этап 5: Java/Kotlin adapter
1. `java/` sibling-папка.
2. lockfile-first стратегия.
3. SCM extraction + sync/status/check.

---

## 10. Definition of Done для каждого нового языка

Реализация считается готовой к alpha, если:
1. Есть рабочие `init/sync/status/check`.
2. Есть `_INDEX.md`, `_SUMMARY.md`, `.aifd-meta.toml`.
3. `check` корректно выставляет exit code.
4. Есть unit-тесты для registry parsing + resolver.
5. Есть integration/smoke тест минимум на 1 fixture проект.
6. Есть CI workflow и README quick-start.

---

## 11. Критерии для выноса в отдельный репозиторий

Вынос sibling-версии в отдельный repo выполняется, когда:
- API/CLI контракт стабилен;
- CI стабилен минимум N недель;
- есть релизный процесс;
- есть документация поддержки и roadmap;
- синхронизация с core-политикой (cache/status/output semantics) зафиксирована.

До этого момента допускается разработка внутри монорепозитория для ускоренного паритета.

---

## 12. Рекомендации по именованию sibling-проектов

- `cargo-ai-fdocs` (Rust)
- `npm-ai-fdocs` (Node/NPM)
- `pypi-ai-fdocs` или `python-ai-fdocs` (Python)
- `composer-ai-fdocs` (PHP)
- `go-ai-fdocs` (Go)
- `nuget-ai-fdocs` (Dotnet)
- `maven-ai-fdocs` (Java)

Единый бренд: **AI Fresh Docs**.


## 13. VS Code extension module (draft v0.1)

Цель: упаковать текущий CLI-контур в удобный UX для VS Code без дублирования бизнес-логики.

### 13.1 Принцип
- Extension НЕ реализует sync-логику заново.
- Extension вызывает установленный CLI (`cargo-ai-fdocs` / `npm-ai-fdocs` / другие sibling-CLI) и отображает результаты.

### 13.2 Команды extension
- `aiFreshDocs.init`
- `aiFreshDocs.sync`
- `aiFreshDocs.status`
- `aiFreshDocs.check`
- `aiFreshDocs.openIndex`

### 13.3 UX-поведение
- Output channel с логами выполнения.
- Status bar item:
  - `AI Docs: Synced`
  - `AI Docs: Outdated`
  - `AI Docs: Error`
- Diagnostics/Problems на основе `check --format json`.
- Quick action: "Run ai-fdocs sync" при обнаружении drift.

### 13.4 Activation events
- `onCommand:aiFreshDocs.*`
- `workspaceContains:**/ai-fdocs.toml`
- (опционально) watcher lockfile (`Cargo.lock`, `package-lock.json`, etc.).

### 13.5 Важный контракт между CLI и extension
- Extension опирается на machine-readable вывод (`status/check --format json`).
- Любое изменение JSON-схемы должно сопровождаться:
  1. обновлением схемы/адаптера в extension,
  2. обновлением этого ТЗ,
  3. записью в changelog/release notes.

### 13.6 Публикация
- Отдельный репозиторий/пакет VS Code extension после стабилизации CLI-контракта.
- На раннем этапе допустим monorepo-подход.

---

## 14. Governance: правило консистентности (обязательно)

Если меняем core-библиотеку/контракт (команды, статус-коды, JSON-формат, структуру output, metadata-схему), **обязательно синхронно обновляем**:
1. `Manifest.md` (стратегия и контракт);
2. `LANGUAGE_EXPANSION_TECH_SPEC.md` (ТЗ для других языков);
3. документацию sibling-реализаций (например, `npn/README.md`, `npn/ROADMAP.md`);
4. спецификацию/код VS Code extension (если изменение затрагивает CLI/API).

Это требование нужно, чтобы не терять консистентность между реализациями и инструментами.
