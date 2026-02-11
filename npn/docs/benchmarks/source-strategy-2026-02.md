# Source strategy benchmark (2026-02)

Дата генерации: 2026-02-11T13:48:16.016Z.

## Как воспроизвести

1. Перейти в каталог `npn/`.
2. Выполнить `npm run benchmark:source-strategy`.
3. Скрипт создаёт временные benchmark-проекты, запускает `sync` в режимах `docs_source=github` и `docs_source=npm_tarball`, затем сохраняет этот отчёт в `npn/docs/benchmarks/source-strategy-2026-02.md`.

## Корпус (реальные npm-пакеты, 22 шт.)

| # | package | version | repo |
|---:|---|---|---|
| 1 | react | 18.3.1 | facebook/react |
| 2 | vue | 3.4.27 | vuejs/core |
| 3 | lodash | 4.17.21 | lodash/lodash |
| 4 | axios | 1.7.2 | axios/axios |
| 5 | express | 4.19.2 | expressjs/express |
| 6 | typescript | 5.6.3 | microsoft/TypeScript |
| 7 | vite | 5.4.10 | vitejs/vite |
| 8 | next | 14.2.15 | vercel/next.js |
| 9 | chalk | 5.3.0 | chalk/chalk |
| 10 | commander | 12.1.0 | tj/commander.js |
| 11 | zod | 3.23.8 | colinhacks/zod |
| 12 | rxjs | 7.8.1 | ReactiveX/rxjs |
| 13 | dayjs | 1.11.13 | iamkun/dayjs |
| 14 | date-fns | 3.6.0 | date-fns/date-fns |
| 15 | prettier | 3.3.3 | prettier/prettier |
| 16 | eslint | 9.12.0 | eslint/eslint |
| 17 | jest | 29.7.0 | jestjs/jest |
| 18 | vitest | 1.6.0 | vitest-dev/vitest |
| 19 | pinia | 2.1.7 | vuejs/pinia |
| 20 | nuxt | 3.13.2 | nuxt/nuxt |
| 21 | svelte | 4.2.19 | sveltejs/svelte |
| 22 | tailwindcss | 3.4.13 | tailwindlabs/tailwindcss |

## Сводные метрики

| mode | synced | cached | skipped | errors | success rate | duration (ms) | useful files |
|---|---:|---:|---:|---:|---:|---:|---:|
| github | 0 | 0 | 0 | 22 | 0.0% | 7079 | 0 |
| npm_tarball | 0 | 0 | 0 | 22 | 0.0% | 3514 | 0 |

## Ошибки по классам

| Error class | github | npm_tarball |
|---|---:|---:|
| NETWORK | 22 | 22 |

## Примечания

- `success rate` = (synced + cached) / 22.
- `useful files` = сумма `files` из `.aifd-meta.toml` по всем синхронизированным пакетам.
