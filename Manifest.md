# AI Fresh Docs (cargo-ai-fdocs) — Manifest (v0.1-alpha, updated)

> Отдельный детальный план интеграции latest-docs: `MANIFEST_DOCSRS_LATEST.md`.
> API-контракт интеграции: `docs/API_CONTRACT.md`.

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

Поддерживается сценарий Rust-экосистемы (папка `fdocs/rust`).

---


### Repository note: npm sibling implementation
- В папке [`npn/`](./npn) находится NPM-версия библиотеки: **npm-ai-fdocs** (Node.js/TypeScript).
- Для `npn/` действует цель функционального паритета с основной реализацией AI Fresh Docs с поправкой на экосистему NPM.
- Должны быть унифицированы: набор команд (`init/sync/status/check`), принципы структуры выходных папок и общая модель статусов/кеша.

## 3. CLI и команды (alpha)

Текущий рабочий сценарий:

```bash
cargo ai-fdocs sync
cargo ai-fdocs sync --force
cargo ai-fdocs status
```

> Примечание: продукт называется **AI Fresh Docs**, а cargo-subcommand в проекте — `ai-fdocs`.

---

## 4. Конфигурация (текущий формат)

Файл: `ai-fdocs.toml` в корне проекта.

```toml
[settings]
output_dir = "fdocs/rust"
max_file_size_kb = 200
prune = true
sync_concurrency = 8

[crates.axum]
repo = "tokio-rs/axum"
ai_notes = "Web framework layer"

[crates.sqlx]
repo = "launchbadge/sqlx"
files = ["README.md", "CHANGELOG.md", "docs/migration-guide.md"]
ai_notes = "Use sqlx::query! where possible"
```

### Семантика полей
- `settings.output_dir` — куда сохранять docs.
- `settings.max_file_size_kb` — лимит размера файла с обрезкой.
- `settings.prune` — удалять устаревшие папки версий.
- `settings.sync_concurrency` — количество параллельных sync-воркеров (по умолчанию `8`).
- `crates.<name>.repo` — источник документации в формате `owner/repo`.
- `crates.<name>.subpath` — опциональный префикс для monorepo (для дефолтных файлов).
- `crates.<name>.files` — явный список файлов (если не указан, используются дефолтные).
- `crates.<name>.ai_notes` — заметки для AI в индексах.

Legacy-формат `sources = [{ type = "github", repo = "..." }]` остаётся поддержанным для обратной совместимости.

---

## 5. Алгоритм `sync` (alpha-контракт)

1. Прочитать `ai-fdocs.toml`.
2. Прочитать `Cargo.lock` и получить `crate -> version`.
3. (Опционально) выполнить pruning.
4. Для каждого crate из конфига:
   - проверить, есть ли версия в lock;
   - проверить кэш (`crate@version` + `.aifd-meta.toml` + fingerprint конфигурации `repo/subpath/files`);
   - определить git ref (тег, иначе fallback на branch);
   - скачать нужные файлы;
   - обработать CHANGELOG;
   - сохранить файлы и метаданные.
5. Перегенерировать `fdocs/rust/_INDEX.md`.
6. Показать итог (`synced/cached/skipped/errors`).

Важно: ошибки по отдельным крейтам/файлам не валят весь sync целиком — обработка best-effort, чтобы остальная документация продолжала обновляться.

---

## 6. Выходная структура

```text
fdocs/rust/
├── _INDEX.md
├── axum@0.8.1/
│   ├── .aifd-meta.toml
│   ├── _SUMMARY.md
│   ├── README.md
│   └── CHANGELOG.md
└── sqlx@0.8.2/
    ├── .aifd-meta.toml
    ├── _SUMMARY.md
    ├── README.md
    └── docs__migration-guide.md
```

---

## 7. Сетевой слой

- Поддержка токена: `GITHUB_TOKEN` / `GH_TOKEN`.
- Без токена — warning про лимиты GitHub API.
- Fallback на default branch, если тег версии не найден.

---

## 8. Roadmap до стабильной рабочей версии

### Ближайший этап (hardening alpha -> beta)
Статус (текущее состояние):
- ✅ retry/backoff и базовая классификация сетевых ошибок реализованы.
- ✅ `check --format json` и CI-рецепты оформлены.
- ✅ кроссплатформенный smoke CI (Linux/macOS/Windows) добавлен.
- ✅ policy совместимости/semver зафиксирована в `COMPATIBILITY.md`.
- ⏳ остаётся: интеграционные сценарии (lockfile/fallback/partial failures) и UX `_INDEX.md` для больших графов.
- ⏳ остаётся: рефакторинг `save_crate_files` (`too_many_arguments`).

- Надёжность сети: retry/backoff для GitHub API и raw-download, явная классификация ошибок (auth/rate-limit/not-found/network).
- Тестовое покрытие: интеграционные сценарии для lockfile-resolve, fallback на branch, частичные ошибки (best-effort).
- Наблюдаемость: более детальная итоговая статистика `sync` по типам ошибок и источникам.

### v0.2 (CI-first)
- Довести `cargo ai-fdocs check` до стабильного CI-контракта (детерминированные exit codes).
- Добавить machine-readable вывод (`--format json`) для интеграции с CI/reporting.
- Подготовить готовые рецепты GitHub Actions (sync/check + cache).

В CI-режиме `check` должен показывать причины по каждому проблемному crate; в GitHub Actions — дополнительно печатать `::error` аннотации.

Рецепты для GitHub Actions:

Минимальный `check`:
```yaml
- uses: actions/checkout@v4
- uses: dtolnay/rust-toolchain@stable
- uses: Swatinem/rust-cache@v2
- run: cargo ai-fdocs check --format json
```

Плановый `sync` (manual/schedule) с авто-коммитом обновлённых docs:
```yaml
- uses: actions/checkout@v4
- uses: dtolnay/rust-toolchain@stable
- uses: Swatinem/rust-cache@v2
- env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  run: cargo ai-fdocs sync
- run: |
    git config user.name "github-actions[bot]"
    git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
    git add fdocs/rust ai-fdocs.toml
    git diff --cached --quiet || git commit -m "chore: refresh ai-fdocs"
- run: git push
```

Вариант с явным cache key (`actions/cache`) для `~/.cargo/*` и `target`:
```yaml
- uses: actions/cache@v4
  with:
    path: |
      ~/.cargo/registry
      ~/.cargo/git
      target
    key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    restore-keys: |
      ${{ runner.os }}-cargo-
```

JSON-контракт `status/check --format json`:
- `summary.{total,synced,missing,outdated,corrupted}`
- `statuses[].{crate_name,lock_version,docs_version,status,reason}`
- `status in {Synced,SyncedFallback,Outdated,Missing,Corrupted}`

### v0.3 (stability envelope)
- Для `.aifd-meta.toml` введена схема `schema_version = 1`; legacy-мета без версии мигрируется при чтении, а более новые неизвестные версии считаются несовместимыми.
- ✅ Улучшен UX `_INDEX.md` для больших dependency graph (навигация, секции, подсказки для AI).
- ✅ Сообщения CLI унифицированы по всем подкомандам (`sync/status/check/init`).

### v1.0 (stable)
- Финальная стабилизация CLI и формата выходных данных (semver promises).
- Кроссплатформенные smoke/regression прогоны (Linux/macOS/Windows).
- Публичная policy-документация: поддерживаемые версии Rust/OS и правила обратной совместимости.

### Техдолг инструментов (ближайший рефакторинг)
- ✅ Закрыт `too_many_arguments` для `storage::save_crate_files` через декомпозицию API (`SaveRequest`).


### Статус (текущее состояние)
- ✅ Retry/backoff и классификация ошибок (auth/rate-limit/not-found/network) внедрены.
- ✅ CI-контракт `check` + `--format json` + готовые GitHub Actions recipes задокументированы.
- ✅ Кроссплатформенный smoke matrix (Linux/macOS/Windows) и policy совместимости зафиксированы.
- ✅ Интеграционные сценарии покрывают fallback на branch и partial failures fetch-пайплайна.
- ✅ Техдолг `too_many_arguments` вокруг `storage::save_crate_files` снят рефакторингом API.
- ✅ Сообщения CLI приведены к единому формату для `sync/status/check/init`.

### После v1.0
- Расширение экосистемы sibling-проектами (Node/npm и далее).
- docs.rs как дополнительный источник с приоритетной стратегией merge/fallback.

### Consistency rule (cross-language + VS Code)
- При изменениях core-контракта библиотеки (CLI-команды, JSON-форматы `status/check`, структура output, схема метаданных) необходимо одновременно обновлять:
  - `Manifest.md`,
  - `LANGUAGE_EXPANSION_TECH_SPEC.md`,
  - документы sibling-реализаций по языкам,
  - спецификацию/реализацию VS Code-модуля.
- Цель правила: не допускать расхождения между Rust/NPM и следующими языковыми версиями, а также между CLI и редакторной интеграцией.

### Decision note (Node/npm source strategy)
- Вопрос о полном отказе от GitHub в npm-версии отмечен как открытый для отдельного обсуждения.
- Текущее решение: **оставляем как есть** (GitHub-путь как основной, npm tarball как экспериментальный opt-in), чтобы не менять стабильный поток без согласования.
- Аргумент для обсуждения за упрощение: парсить один источник проще, чем два.

---

### Safety policy: donor outage tolerance
- Под "донорами" понимаются внешние источники документации (GitHub API/raw, registry API, tarball-архивы).
- При их недоступности инструмент должен переходить в degraded-режим без влияния на основную платформу:
  - не изменять/ломать исходный код проекта,
  - не прерывать весь sync из-за одной зависимости,
  - сохранять локальный кеш и выдавать понятную диагностику.
- Ошибки доступа к донорам должны отражаться в `status/check` и CI-контракте, но не приводить к поломке runtime приложения.

## 9. Definition of Done для alpha

Считаем alpha готовой, когда:
- проект стабильно собирается,
- `sync` зеркалит lock-версии в выходной папке,
- `status` корректно сигнализирует `synced/outdated/missing`,
- сетевые и файловые ошибки даются понятными сообщениями.
