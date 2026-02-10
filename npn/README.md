# ai-fdocs (NPM) v0.2

Node.js/TypeScript версия `ai-fdocs` с паритетом ключевых фич Rust v0.2:

- `init` из `package.json` (прямые зависимости) + npm registry;
- `sync` с параллельной загрузкой (`MAX_CONCURRENT=8`);
- `check` для CI (exit code 0/1);
- `_SUMMARY.md` в каждой папке пакета;
- `config_hash` для автоматического invalidation кеша;
- улучшенный `status` с подсказками.

## Экспериментальный режим источника docs

По умолчанию документация тянется из GitHub-репозитория пакета.

Можно включить экспериментальный режим, который тянет docs из npm tarball:

```toml
[settings]
experimental_npm_tarball = true
```

> ⚠️ Это экспериментальный режим: может вести себя иначе для нестандартных пакетов.

## Безопасность и поведение при недоступности источников

`npm-ai-fdocs` должен работать в безопасном degraded-режиме, если источники docs
временно недоступны (GitHub/npm registry):

- не ломать приложение и исходный код проекта;
- не падать целиком из-за одного проблемного пакета (best-effort);
- сохранять ранее скачанный кеш;
- явно показывать ошибки в `status/check` и CI.

Итог: при сетевых проблемах ухудшается свежесть документации, но не стабильность платформы.

## Быстрый старт

```bash
npm install
npm run build
node dist/cli.js --help
```

## Команды

- `ai-fdocs init [--overwrite]`
- `ai-fdocs sync [--force]`
- `ai-fdocs status`
- `ai-fdocs check`
## План развития

Подробная дорожная карта: [`ROADMAP.md`](./ROADMAP.md).
