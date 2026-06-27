<p align="center">
  <img src="minerpulse-desktop/static/logo.png" width="96" alt="Miner Pulse logo" />
</p>

<h1 align="center">Miner Pulse</h1>

<p align="center">
  <strong>Мониторинг ASIC-майнеров на рабочем столе</strong><br />
  WhatsMiner · Antminer · Avalon — опрос, логи, графики, карта чипов
</p>

<p align="center">
  <a href="https://github.com/BobJustFry/MinerPulse/releases/latest">Скачать для Windows</a>
  ·
  <a href="https://t.me/miner_pulse">Telegram</a>
  ·
  <a href="#поддержать-проект">Донат</a>
</p>

<p align="center">
  <img src="https://img.shields.io/github/v/release/BobJustFry/MinerPulse?label=version" alt="Release" />
  <img src="https://img.shields.io/badge/platform-Windows-blue" alt="Windows" />
  <img src="https://img.shields.io/badge/stack-Tauri%20%2B%20Rust%20%2B%20Svelte-646cff" alt="Stack" />
</p>

---

## О программе

**Miner Pulse** — настольное приложение для инженеров и операторов майнинг-ферм. Подключается к майнеру по IP, читает телеметрию в реальном времени, строит графики, показывает карту чипов и помогает разбирать логи.

Интерфейс на **русском**, **английском** и **китайском**. Светлая и тёмная темы.

## Скриншоты

### Панель данных

Хешрейт, температуры, вентиляторы, шары и платы — всё на одном экране.

<p align="center">
  <img src="docs/screenshots/dashboard.png" alt="Панель данных Antminer L7" width="920" />
</p>

### Графики

Опрос с настраиваемой частотой. Режимы «Плитка» и «Список» — удобно на широком мониторе.

<p align="center">
  <img src="docs/screenshots/charts.png" alt="Графики хешрейта, температур и вентиляторов" width="920" />
</p>

### Карта чипов

Температура, напряжение и статистика по каждому чипу на плате (WhatsMiner / Antminer).

<p align="center">
  <img src="docs/screenshots/chips.png" alt="Матрица чипов по платам" width="920" />
</p>

### О программе

Проверка обновлений, ссылки на GitHub и Telegram, информация о версии.

<p align="center">
  <img src="docs/screenshots/about.png" alt="Окно О программе" width="420" />
</p>

## Возможности

| Раздел | Что умеет |
|--------|-----------|
| **Данные** | Модель, прошивка, аптайм, хешрейт (5s / avg / RT), температуры, RPM, шары, HW-ошибки, платы |
| **Чипы** | Матрица чипов: температура, напряжение, решения |
| **Консоль** | Сырой лог / ответ API |
| **Пулы** | Активные и резервные пулы, статус |
| **Графики** | Хешрейт, температуры плат, скорость вентиляторов; запись и воспроизведение сессий |
| **Команды** | Отправка команд майнеру (Service) |
| **Поиск** | Сканирование подсети, выбор майнера из списка |
| **Импорт** | Открытие `.mpulse` логов и сессий, drag & drop |
| **Обновления** | Подписанный авто-апдейтер из GitHub Releases |

### Поддерживаемые семейства

- **WhatsMiner** — API v2/v3, карта чипов, коды ошибок
- **Antminer** — cgminer API (L7, E9 и др.)
- **Avalon** — TCP-команды

## Установка

1. Откройте [Releases](https://github.com/BobJustFry/MinerPulse/releases/latest).
2. Скачайте **`MinerPulse_*_x64-setup.exe`**.
3. Установите (NSIS, права администратора — см. [SECURITY.md](SECURITY.md)).
4. Запустите, введите IP и порт майнера (обычно `4028`), нажмите **Читать** или **Опрос**.

> Версия и номер сборки отображаются в заголовке окна: `Miner Pulse X.Y.Z (BBB)`.

## Поддержать проект

Разработка ведётся в свободное время. Если Miner Pulse помогает вам в работе — можно поблагодарить:

```
USDT TRC20: TAQLsXQA7WzNfoCTHvXxj8yFBXTJRKz99w
```

Тот же адрес указан в приложении: **О программе → Поддержать проект**.

## Для разработчиков

**Windows:**

```powershell
cd minerpulse-desktop
npm install
npm run dev:app
```

| Поле | Файл | Правило |
|------|------|---------|
| Версия `X.Y.Z` | `VERSION.json` | Меняется **только с одобрения владельца** |
| Сборка `BBB` | `VERSION.json` | `node scripts/bump-build.mjs` после каждого изменения |

Подробнее: [.cursor/rules/minerpulse-strict.mdc](.cursor/rules/minerpulse-strict.mdc) · [REPOSITORY.md](REPOSITORY.md) · [LICENSE](LICENSE)

## Структура репозитория

```
minerpulse-core/       Rust: драйверы, TCP, снимки, импорт
minerpulse-desktop/    Tauri + Svelte UI
docs/screenshots/      Скриншоты для README
releases/              update.json — манифест авто-обновления
scripts/               bump build / sync version
```

## Контакты

- **GitHub:** [BobJustFry/MinerPulse](https://github.com/BobJustFry/MinerPulse)
- **Telegram:** [@miner_pulse](https://t.me/miner_pulse)
- **Разработчик:** Bobrov Andrey

---

<p align="center"><sub>Miner Pulse · proprietary · see <a href="LICENSE">LICENSE</a></sub></p>
