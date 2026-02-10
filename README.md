# cargo-ai-fdocs

CLI-инструмент для синхронизации version-locked документации зависимостей,
чтобы AI-ассистенты работали с актуальными API вместо устаревшего контекста обучения.

## Что это решает

AI часто «галлюцинирует» по старым версиям библиотек. `cargo-ai-fdocs` подтягивает
README/CHANGELOG/гайды из GitHub и сохраняет их рядом с проектом в структуре,
удобной для подключения в контекст ассистента.

## Текущий статус (alpha)

Поддерживается Rust-пайплайн:
- чтение `Cargo.lock`;
- резолв версий зависимостей;
- загрузка документации из GitHub;
- сохранение в `docs/ai/vendor-docs/rust/<crate>@<version>/`;
- генерация `_INDEX.md`;
- проверка состояния через `status`.

## Команды

```bash
cargo ai-docs sync
cargo ai-docs sync --force
cargo ai-docs status
```

## Конфиг

Создайте `ai-docs.toml` в корне проекта:

```toml
[settings]
output_dir = "docs/ai/vendor-docs/rust"
max_file_size_kb = 200
prune = true

[crates.axum]
sources = [{ type = "github", repo = "tokio-rs/axum" }]
ai_notes = "Use axum extractors and router-first style."

[crates.sea-orm]
sources = [{ type = "github", repo = "SeaQL/sea-orm" }]
files = ["README.md", "CHANGELOG.md", "docs/ORM.md"]
ai_notes = "Prefer SeaORM entities over raw SQL."
```

## Подробная спецификация

См. `Manifest.md` — там актуализированный alpha-манифест, дорожная карта и критерии готовности.
