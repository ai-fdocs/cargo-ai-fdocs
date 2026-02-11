# npm-ai-fdocs Roadmap

Дорожная карта для Node.js/NPM версии **AI Fresh Docs** (`npn/`).

## Цель

Довести `npm-ai-fdocs` до стабильного релиза с паритетом UX/надежности относительно Rust-версии и прозрачной стратегией источника документации.

---

## Текущее состояние (baseline)

Уже реализовано:
- CLI-команды: `init`, `sync`, `status`, `check`.
- `init`: чтение прямых зависимостей из `package.json` + metadata из npm registry.
- `sync`: параллельная загрузка docs, кеш с `config_hash`, генерация `_INDEX.md` и `_SUMMARY.md`.
- `check`: CI-friendly exit code.
- Экспериментальный источник docs из npm tarball (опциональный флаг).
- `.aifd-meta.toml` для Node-версии включает `schema_version = 2` (legacy без поля остаётся совместимым).
- `config_hash` нормализует `subpath` и порядок `files` для стабильной идемпотентности.
- Добавлены unit-тесты для `config_hash` и совместимости metadata/cache.

---

## Прогресс по roadmap (с отметками)

- [x] A2.1 Стандартизировать `.aifd-meta.toml` (`schema_version = 2`).
- [x] A2.3 Стабилизировать `config_hash` (normalize `subpath`, порядок `files`).
- [x] A3 (частично) Unit-тесты для cache/hash совместимости.
- [x] B1.1 `check --format json` (machine-readable).
- [x] A1 (частично) Добавлен общий retry/backoff + классификация HTTP ошибок в network-слое.
- [x] B1.2 GitHub Actions workflow для `npn/**` + fixture `check` job.
- [x] B2 Runbook и token-management рекомендации.
- [x] C1.1 Добавлены source-метрики в `sync` (итоги по активному источнику).
- [x] C1.2 Добавлен machine-readable отчёт `sync --report-format json`.
- [x] C1.3 JSON-режим `sync` очищен до строгого JSON вывода (без лишних логов).
- [x] A1 (доп.) Добавлен short error summary по code-классам в `sync`.
- [x] A3 (доп.) Добавлены unit-тесты для `resolveRef` fallback и рендеринга `_SUMMARY.md`/`_INDEX.md`.
- [x] C1 (доп.) Реализован fallback с GitHub на npm tarball в `sync` при ошибках/пустой выборке GitHub.
- [x] A3 (доп.2) Добавлены unit-тесты `cmdSync` для GitHub→npm tarball fallback сценариев.
- [x] A3 (доп.3) Добавлены unit-тесты для ветки `GitHub fetch error -> npm fallback error` и проверки error-reporting.
- [x] A3 (доп.4) Добавлен тест для skip-ветки с пустым GitHub-результатом и диагностикой падения npm fallback.
- [x] A3 (доп.5) Добавлен unit-тест partial failures (best-effort): один пакет падает, остальные успешно синкаются.
- [x] A3 (доп.6) Добавлен тест на отсутствие повторного npm fallback-запроса после уже выполненного fallback с пустым результатом.

---

## Milestone A — Stabilize v0.2.x (ближайший)

**Задача:** снять риски по надежности и предсказуемости, не меняя основной UX.

### A1. Надёжность network/fetch
- Единая стратегия retry/backoff для:
  - npm registry API,
  - GitHub API,
  - raw-content/tarball download.
- Явная классификация ошибок (`auth`, `rate_limit`, `not_found`, `network`, `parse`).
- Детальные сообщения в `sync/check` + человекочитаемый short summary.

### A2. Качество кеша и idempotency
- Стандартизировать `.aifd-meta.toml` (добавить `schema_version` для Node-версии).
- Явно документировать поведение legacy metadata без `config_hash`.
- Проверить стабильность `config_hash` (порядок `files`, normalize `subpath`).

### A3. Тестирование
- Unit + integration набор для:
  - tag->fallback branch,
  - partial failures (best-effort),
  - cache hit/miss при изменении `repo/subpath/files`,
  - корректность `_SUMMARY.md` / `_INDEX.md`.
- Минимальные e2e smoke сценарии на fixture-проектах.

**Definition of Done (A):**
- repeatable `sync` без ложных перекачек;
- `check` предсказуемо выявляет drift;
- покрыты критические сценарии fetch/cache.

---

## Milestone B — CI/Automation v0.3

**Задача:** сделать npm-версию удобной для production CI.

### B1. CI contract
- `check --format json` (machine-readable результат).
- GitHub Actions workflow для `npn/**`:
  - install/build/test,
  - отдельный job для `check` на fixture-проекте.

### B2. Документация и операционная готовность
- Runbook для команды: как дебажить 429/401/404.
- Рекомендации по token management (`GH_TOKEN`/`GITHUB_TOKEN`).
- Примеры `.gitattributes` и минимального CI pipeline.

**Definition of Done (B):**
- готовые CI-рецепты, которые можно копировать в сторонние репозитории;
- машинный формат статуса для отчётности.

---

## Milestone C — Source strategy decision (архитектурный)

**Главный открытый вопрос:** оставляем 2 источника (GitHub + npm tarball) или переходим на 1 источник.

### Вариант 1: GitHub primary + npm tarball fallback (текущий)
Плюсы:
- проще находить «живые» docs в репозитории,
- хорошо работает для monorepo и нестандартных layout.

Минусы:
- 2 внешних источника = выше сложность и больше edge cases.

### Вариант 2: npm tarball primary (single-source)
Плюсы:
- проще архитектура (один домен и один тип артефакта),
- контент ближе к реально опубликованному пакету.

Минусы:
- в tarball может не быть полной документации,
- сложнее сопоставлять docs, которые лежат только в GitHub repo.

### План принятия решения
1. Добавить метрики источников в `sync` (сколько пакетов успешно на каждом источнике).
2. Прогнать выборку реальных проектов (минимум 20) и сравнить:
   - coverage (сколько полезных docs файлов),
   - стабильность/ошибки,
   - скорость.
3. Зафиксировать decision record и обновить default source.

**Definition of Done (C):**
- принято формальное решение по default source;
- это решение отражено в README + config docs + migration notes.

---

## Milestone D — v1.0 release

**Задача:** стабилизировать API/поведение и подготовить перенос в отдельный репозиторий.

- Freeze CLI contract и формат metadata.
- SemVer policy + compatibility matrix (Node LTS, OS).
- Changelog discipline и release checklist.
- Отдельный репозиторий `npm-ai-fdocs` с перенесённой историей/доками.

**Definition of Done (D):**
- релиз `1.0.0` с зафиксированными контрактами;
- команда может поддерживать проект без «скрытых» ручных шагов.

---

## Приоритеты на ближайшие 2 спринта

1. A1/A2/A3 (стабилизация + тесты).
2. B1 (`check --format json`) и CI recipes.
3. Сбор метрик для C (source strategy decision).

