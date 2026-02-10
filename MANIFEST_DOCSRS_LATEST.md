# MANIFEST: docs.rs + crates.io latest-docs integration (v0.1 draft)

> Цель: реализовать **рабочий** и **стабильный** режим синка самой свежей документации для Rust crates без «фантазийных» API-решений.

## 0) Контекст и ограничения

- Текущая реализация синка уже стабильна для GitHub-файлов и lockfile-версий.
- Нужен отдельный контур для **latest docs**:
  - источник версии: `crates.io`;
  - источник контента документации: `docs.rs`;
  - fallback: GitHub (README/CHANGELOG/guides), если docs.rs ещё не готов или недоступен.

---

## 1) Definition of Success (что считаем «интеграция работает»)

- [ ] `cargo ai-fdocs sync --mode latest-docs` синхронизирует документацию для latest-версий всех настроенных crates.
- [ ] Для каждого crate сохраняется мета с источником/версией/временем проверки/статусом fallback.
- [ ] `cargo ai-fdocs status` и `check` корректно оценивают режим latest-docs (без ложных `Outdated` из-за lock-version mismatch).
- [ ] Повторный запуск при отсутствии upstream-изменений не перекачивает всё заново (TTL + conditional check).
- [ ] Набор тестов покрывает happy path, отказ docs.rs, отказ crates.io, частичные ошибки, cache invalidation.
- [ ] Документация (README + пример конфига + отдельный раздел про API-контракт) обновлена.

---

## 2) План работ по этапам

## Этап A — API-контракт и архитектура (перед кодом)

- [x] A1. Зафиксировать источник истины по latest-версии: `crates.io`.
- [x] A2. Зафиксировать источник docs-контента: `docs.rs`.
- [x] A3. Зафиксировать fallback-цепочку: `docs.rs -> GitHub`.
- [x] A4. Описать endpoint-контракт (request/response, retry, лимиты, таймауты, коды ошибок) в `docs/API_CONTRACT.md`.
- [x] A5. Зафиксировать политику свежести (TTL, revalidation, force refresh).
- [x] A6. Зафиксировать политику деградации (какие ошибки фатальны, какие non-fatal с best-effort).

**Критерий завершения этапа A:** есть документированный и согласованный контракт, по которому можно писать код без неопределённостей.

---

## Этап B — Конфиг и модель данных

- [ ] B1. Добавить в конфиг режим синка:
  - `sync_mode = "lockfile" | "latest_docs"`.
- [ ] B2. Добавить настройки свежести:
  - `latest_ttl_hours`;
  - `docsrs_single_page = true` (MVP по умолчанию).
- [ ] B3. Обновить валидацию конфига:
  - разрешить crates с `docsrs` источником без обязательного `repo`.
- [ ] B4. Расширить `.aifd-meta.toml`:
  - `source_kind`, `sync_mode`, `upstream_latest_version`, `upstream_checked_at`, `ttl_expires_at`.
- [ ] B5. Обновить fingerprint кэша, чтобы учитывались новые поля режима.

**Критерий завершения этапа B:** конфиг выражает latest-docs режим полностью, мета и кэш корректно различают стратегии.

---

## Этап C — Fetcher-слой

- [ ] C1. Реализовать `CratesIoFetcher`:
  - получение latest stable версии;
  - классификация ошибок (auth/rate-limit/network/not-found/other).
- [ ] C2. Реализовать `DocsRsFetcher`:
  - получение одностраничного снимка docs (MVP);
  - нормализация и сохранение source URL.
- [ ] C3. Реализовать fallback в GitHub-fetcher (reuse существующего).
- [ ] C4. Унифицировать ошибки в существующую `AiDocsError`/`SyncErrorKind`.
- [ ] C5. Добавить retry/backoff в новых fetcher-ах по аналогии с GitHub fetcher.

**Критерий завершения этапа C:** fetcher-слой стабильно получает latest версию + docs-контент и корректно деградирует.

---

## Этап D — Sync orchestration

- [ ] D1. В `run_sync` внедрить выбор стратегии по `sync_mode`.
- [ ] D2. Для `latest_docs` делать flow:
  1) latest from crates.io;
  2) fetch docs from docs.rs;
  3) fallback GitHub (если настроен и docs.rs failed);
  4) save with mode/source meta.
- [ ] D3. Сохранить текущую lockfile-логику без regressions.
- [ ] D4. Учесть параллелизм и лимиты, чтобы не перегружать docs.rs.
- [ ] D5. Обновить статистику sync-результата по источникам (`docsrs/github/fallback`).

**Критерий завершения этапа D:** обе стратегии работают, совместимы и прозрачно диагностируются.

---

## Этап E — Status/Check semantics

- [ ] E1. `status/check` должны быть mode-aware:
  - lockfile: сравнение с lock version;
  - latest_docs: сравнение с upstream latest.
- [ ] E2. Добавить reason-коды для latest-docs (ttl expired, upstream changed, fallback used).
- [ ] E3. JSON-формат дополнить полями mode/source.
- [ ] E4. Не допускать ложных `Outdated` при latest_docs.

**Критерий завершения этапа E:** статус корректен и однозначен для CI в обоих режимах.

---

## Этап F — Storage layout и путь `docs/fdocs`

- [ ] F1. Сменить рекомендуемый root output на `docs/fdocs`.
- [ ] F2. Сохранить плоскую структуру `crate@version/` (без лишней вложенности).
- [ ] F3. Для latest-docs сохранять минимум:
  - `API.md` (или `API.html`),
  - `_SUMMARY.md`,
  - `.aifd-meta.toml`.
- [ ] F4. Обновить `_INDEX.md` генерацию под mixed sources.
- [ ] F5. Сохранить совместимость с существующими flatten-правилами файлов.

**Критерий завершения этапа F:** структура простая (`docs/fdocs/*`) и совместима с текущими механизмами хранения.

---

## Этап G — Тесты (обязательный блок)

- [ ] G1. Unit tests для:
  - парсинга нового конфига,
  - fingerprint/meta migration,
  - mode-aware status rules.
- [ ] G2. Integration tests для sync latest-docs:
  - happy path;
  - docs.rs unavailable -> GitHub fallback;
  - crates.io unavailable;
  - partial success и best-effort.
- [ ] G3. Regression tests для lockfile mode (чтобы не сломать текущую ветку).
- [ ] G4. Snapshot tests для `_INDEX.md` и `_SUMMARY.md`.
- [ ] G5. CI matrix green на Linux/macOS/Windows.

**Критерий завершения этапа G:** тесты фиксируют поведение и предотвращают «интеграцию в пустоту».

---

## Этап H — Документация и эксплуатация

- [ ] H1. Обновить `README.md`:
  - новый режим latest-docs;
  - путь `docs/fdocs`;
  - примеры команд и ожидаемой структуры.
- [ ] H2. Обновить `examples/ai-docs.toml` под новую конфигурацию.
- [ ] H3. Добавить troubleshooting раздел:
  - rate limit,
  - docs.rs lag,
  - fallback поведение.
- [ ] H4. Добавить migration note с прежнего output_dir.
- [ ] H5. Обновить `Manifest.md` ссылкой на этот манифест.

**Критерий завершения этапа H:** разработчик может включить режим latest-docs по документации без чтения исходников.

---

## Этап I — Релизный контроль

- [ ] I1. Прогон полного набора тестов и линтеров.
- [ ] I2. Проверка обратной совместимости существующих конфигов.
- [ ] I3. Dry-run на реальном проекте со списком популярных crates.
- [ ] I4. Финальная ревизия ошибок/логов/метрик.
- [ ] I5. Выпуск beta-флага latest-docs (или soft launch).

**Критерий завершения этапа I:** функция готова к безопасному использованию в реальных репозиториях.

---

## 3) Риски и анти-риски

### Риски
- docs.rs не всегда моментально содержит свежую сборку.
- Возможны сетевые/лимитные проблемы и нестабильные ответы upstream.
- Смешение lockfile-логики и latest-логики может запутать `status/check`.

### Анти-риски (что делаем заранее)
- TTL + conditional revalidation вместо агрессивной перекачки.
- Чёткая mode-aware семантика `status/check`.
- Fallback на GitHub с явной маркировкой в мета и summary.
- Тесты деградации как обязательная часть DoD.

---

## 4) Предлагаемая целевая структура хранения (MVP)

```text
docs/fdocs/
├── _INDEX.md
├── serde@1.0.228/
│   ├── .aifd-meta.toml
│   ├── _SUMMARY.md
│   └── API.md
├── tokio@1.48.0/
│   ├── .aifd-meta.toml
│   ├── _SUMMARY.md
│   └── API.md
└── sqlx@0.8.2/
    ├── .aifd-meta.toml
    ├── _SUMMARY.md
    ├── API.md
    └── CHANGELOG.md    # если получен через fallback/repo files
```

---

## 4.1) Virtual acceptance walkthrough (nothing missed)

- [x] Проверяем режим и валидацию конфига до сети.
- [x] Разделяем ответственность источников: crates.io (version), docs.rs (content), GitHub (fallback).
- [x] Фиксируем TTL/refresh/fallback поведение в явном контракте.
- [x] Фиксируем mode-aware `status/check` без ложных `Outdated`.
- [x] Фиксируем обязательные unit/integration/regression тесты.
- [x] Добавляем reason-code матрицу для CI и дебага.
- [x] Добавляем reliability checklist (atomic write, retries, deterministic outputs).

---

## 5) Журнал выполнения

- [x] Создан отдельный манифест интеграции latest-docs.
- [x] Зафиксированы этапы A-I с критериями завершения.
- [x] Добавлены чек-листы по логике, тестам, документации, релизному контролю.
- [ ] Начата кодовая реализация (следующий PR).

