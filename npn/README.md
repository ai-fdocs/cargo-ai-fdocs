# ai-fdocs (NPM) v0.2

Node.js/TypeScript версия `ai-fdocs` с паритетом ключевых фич Rust v0.2:

- `init` из `package.json` (прямые зависимости) + npm registry;
- `sync` с параллельной загрузкой (`MAX_CONCURRENT=8`);
- `check` для CI (exit code 0/1);
- `_SUMMARY.md` в каждой папке пакета;
- `config_hash` для автоматического invalidation кеша;
- улучшенный `status` с подсказками.

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
