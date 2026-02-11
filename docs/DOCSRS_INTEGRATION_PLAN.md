# План интеграции с docs.rs (фаза запуска)

> Статус: **kickoff**. Этот план фиксирует ближайшие шаги для старта реализации режима `latest-docs`.

## Цель фазы

Запустить end-to-end поток `crates.io -> docs.rs -> local artifacts` для `cargo ai-fdocs sync --mode latest-docs` с безопасной деградацией в GitHub fallback.

> В этой фазе считаем целевым размещение артефактов **в корне библиотеки** (repo root), в `./fdocs/...`, а не в подпапке `./docs/...`.

## Объём первой итерации (Sprint 1)

1. **Каркас источников latest-docs**
   - добавить HTTP-клиент к `crates.io` для получения latest версии crate;
   - добавить HTTP-клиент к `docs.rs` для загрузки входной страницы `https://docs.rs/crate/{crate}/{version}`;
   - определить и нормализовать ошибки (fallback-eligible vs fatal).

2. **Нормализация контента в `API.md`**
   - извлечь основной rustdoc-контент из HTML;
   - конвертировать в markdown (минимум: H1/Overview/API Reference/code block/footer source URL);
   - применить детерминированную truncate policy по `max_file_size_kb`.

3. **Запись артефактов + мета**
   - сохранять `API.md`, `_SUMMARY.md`, `.aifd-meta.toml`;
   - записывать `artifact_format`, `artifact_path`, `artifact_sha256`, `artifact_bytes`, `truncated`, `docsrs_input_url`;
   - явно маркировать `source_kind=docsrs` или `source_kind=github_fallback`.

4. **Mode-aware pipeline**
   - включить ветвление `sync` по `--mode latest-docs`;
   - сохранить lockfile-flow без изменений;
   - подключить fallback `docs.rs -> GitHub` при eligible ошибках.

5. **Минимальные тесты на запуск фазы**
   - unit: маппинг ошибок docs.rs и crates.io;
   - integration (mock): happy path + docs.rs failure -> GitHub fallback;
   - regression: lockfile mode остаётся зелёным.

## Вне Sprint 1 (следующая итерация)

- TTL/revalidation (`latest_ttl_hours`, conditional checks);
- mode-aware `status/check` с корректной семантикой latest-docs;
- snapshot-тесты для `_INDEX.md`/`_SUMMARY.md`;
- migration/runtime defaults для целевой структуры `./fdocs/*` в корне репозитория.

## Definition of Done для kickoff

- Команда `cargo ai-fdocs sync --mode latest-docs` синхронизирует минимум 1 crate в `API.md` формате.
- При отказе docs.rs и наличии `repo` в конфиге срабатывает GitHub fallback без падения всего sync.
- Мета отражает источник и статус truncation/fallback.
- Есть покрытие базовых happy/degraded сценариев интеграционными тестами.

## Порядок выполнения

1. Источники (`crates.io`, `docs.rs`) + модель ошибок.
2. Нормализация HTML -> `API.md`.
3. Storage/meta для latest-docs.
4. Интеграция в sync mode.
5. Тесты и фиксация поведения.
