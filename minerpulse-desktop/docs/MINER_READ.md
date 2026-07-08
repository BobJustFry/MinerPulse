# Механизм чтения майнера (Miner Pulse desktop)

## UI → Rust

1. Кнопка **Читать** → `readMiner()` в `+page.svelte`.
2. Вызов Tauri `read_miner` с `{ ip, port, whatsminer_auth? }`.
3. **Отмена** → `cancel_read_miner` + инкремент `readGeneration` (старый ответ UI игнорируется).
4. Смена IP → `invalidateRead()` (cancel в Rust + сброс snapshot).

## Rust: одна операция I/O

`MinerIoGate` — mutex: одновременно только **одна** сетевая операция (read / probe / auth test / enable API).

`ReadSession` — при новом `read_miner`:
- выставляет `cancel=true` на предыдущий job;
- новый job получает свой `Arc<AtomicBool>` в `FetchOptions.cancel`.

Таймаут read: **15 с** (`READ_MINER_TIMEOUT`). По таймауту — `cancel=true`, UI получает `CONN_TIMEOUT`.

## Цепочка fetch

```
read_miner
  → read_fetch_options(fast_poll=true, fetch_chips=true, cancel)
  → spawn_blocking + MinerIoGate
  → fetch_with_detect (minerpulse-core)
```

### fast_poll=true (ручное чтение)

**Быстрый путь** (`fetch_with_detect_fast`):
1. `{"cmd":"summary"}` JSON → если WhatsMiner → сразу `WhatsminerDriver::fetch_with_options`.
2. Иначе `stats` + `summary` → Antminer/Avalon.
3. Если не распознан → полный `fetch_with_detect_full` (медленнее).

На каждом шаге: `ensure_not_cancelled` → `OPERATION_CANCELLED`.

### WhatsMiner (`fetch_snapshot_impl`)

| Шаг | Действие | Таймаут (типично) |
|-----|----------|-------------------|
| 1 | TCP `summary`, `edevs` | ~1.5 с на вызов (`TcpCgminerClient::for_read`) |
| 2 | LuCI chips (`fetch_btminer_chip_data`) | 2 с fast (https, admin/admin или cloud creds) |
| 3 | Access probe | `probe_fast` ~2.4 с (API 4433) если chips пустые |

`needs_setup` → UI показывает модалку LuCI.

### Учётные данные

- Явные `whatsminer_auth` из UI → только они.
- Иначе `try_resolve_auth_for_ip` (MAC↔IP кэш + облако).
- Иначе LuCI: `admin/admin`.

## Почему «первый раз висит, второй — модалка за 2 с»

1. **Первое чтение** заняло gate и/или уперлось в таймаут LuCI/TCP (до 15 с). UI: «Чтение…».
2. **Отмена** ставит cancel, но поток мог ещё держать gate до выхода из TCP.
3. **Второе чтение** стартует после освобождения gate; LuCI уже отвалился быстро → telemetry + `needs_setup` → модалка через пару секунд.

Смотреть `minerpulse-diagnostic.log`: строки `read/*`, `detect/*`, `whatsminer/*`.

## Диагностический лог

- Файл: `{app_data}/diagnostics/minerpulse-diagnostic.log`
- Отправка: **О программе → Отправить диагностический лог** (нужен вход в аккаунт)
- ZIP: лог + `manifest.json` (hwid, version, timezone)
- Скачать: [mpulse.bob4.fun](https://mpulse.bob4.fun) → личный кабинет → Диагностические логи
