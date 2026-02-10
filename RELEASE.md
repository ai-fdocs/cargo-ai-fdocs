# Release playbook (`cargo-ai-fdocs`)

Этот чеклист нужен, чтобы выпускать релиз предсказуемо и без пропуска важных шагов.

## 1) Подготовить ветку релиза

```bash
git checkout main
git pull --ff-only
git checkout -b release/vX.Y.Z
```

## 2) Обновить версию

1. Поднять версию в `Cargo.toml` (`[package].version`).
2. При необходимости обновить roadmap/совместимость в `README.md` и `COMPATIBILITY.md`.

## 3) Локально прогнать обязательные проверки

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo check --all-targets --all-features
cargo test --all-targets --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --document-private-items
```

Опционально (как в CI security job):

```bash
cargo install cargo-audit --locked
cargo install cargo-deny --locked
cargo audit
cargo deny check advisories bans licenses sources
```

## 4) Проверить функциональный контракт CLI

Минимум перед релизом:

```bash
cargo run -- check --format json
cargo run -- status --format json
```

Критерии:
- `check` возвращает `0`, когда все crate синхронизированы.
- JSON-схема содержит стабильные поля (`summary`, `statuses`, и поля статусов).

## 5) Проверить CI

Убедиться, что в GitHub Actions зелёные:
- smoke checks (Linux/macOS/Windows),
- rust checks (fmt/clippy/check/test),
- rustdoc,
- security (audit + deny).

## 6) Оформить релизный PR

В PR включить:
- bump версии,
- краткий changelog (breaking/non-breaking),
- подтверждение прохождения всех проверок.

После аппрува — merge в `main`.

## 7) Тег и GitHub Release

```bash
git checkout main
git pull --ff-only
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

Далее создать GitHub Release по тегу `vX.Y.Z` с заметками:
- что изменилось,
- есть ли breaking changes,
- миграционные шаги (если есть).

## 8) Публикация crate (если планируется crates.io)

Проверить dry-run:

```bash
cargo publish --dry-run
```

Публикация:

```bash
cargo publish
```

## 9) Пост-релизная валидация

- Установить релизную версию в чистом окружении.
- Проверить `cargo ai-fdocs --help` и базовый сценарий `sync/check` на тестовом проекте.
- При необходимости быстро выпустить patch `vX.Y.(Z+1)`.

---

## Быстрый one-shot чек перед нажатием "Release"

```bash
cargo fmt --all -- --check \
&& cargo clippy --all-targets --all-features -- -D warnings \
&& cargo check --all-targets --all-features \
&& cargo test --all-targets --all-features \
&& RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --document-private-items
```
