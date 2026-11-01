# bulba music

> **Ревизия 2026-11-01.** Разговор о «заменить Spotify целиком» переопределил клиентскую
> часть. Строки, помеченные `↺`, ниже заменены; всё остальное в силе.

---


Приватный музыкальный сервис в личной сети. 2-3 пользователя, всё в Tailscale,
free-licensed аудио, никакого публичного доступа. Рекомендации — единственная
причина, по которой сервис написан вместо того, чтобы его не писать.

---

## Тезис

**У сигналов прослушивания есть lead time, который нельзя купить.**

Код восстанавливается. Аудио восстанавливается. Каталог, обложки, плейлисты —
всё пересобирается из источника. Данные «что я слушал в июне и где скипнул» —
не восстанавливаются: нельзя вернуться в прошлый июнь и дослушать тогдашние
треки до конца.

Отсюда три следствия, и они определяют весь порядок работ:

1. Схема `play_event` фиксируется в первую неделю, до написания рекомендателя.
2. Кратчайший сквозной срез — «трек в папке → играет на телефоне → событие в
   журнале». Он и запускает часы.
3. Рекомендатель — **вторая итерация, не первая**. Первая итерация собирает
   данные и ничего не рекомендует.

Всё остальное в этом плане можно переставить и переделать. Этот срез — нет.

---

## Зафиксированные решения

| Область | Решение | Почему |
|---|---|---|
| Язык сервера | Rust: `axum` + `tokio` + `sqlx` + `serde` | пользователь выбрал; Go в проекте не нужен |
| Логика клиента | Rust, общий `.so` с сервером | `recommend` — одна реализация, два запуска |
| ↺ NN | `ort` (ONNX Runtime): CLAP/MuQ для эмбеддингов; `burn` остаётся опцией для NN-адаптера | CLAP даёт и audio→audio, и text→audio одним файлом |
| БД | SQLite (WAL) | 20k × 128 f32 = 9.8 MiB, брутфорс-косинус — мс |
| ↺ Транскод | FLAC-мастер + Opus 128 по требованию, в кэш по sha | 280 ГБ FLAC на HDD против 38 ГБ Opus; мастер не пережимаем |
| Загрузка | 3 адаптера → один глубокий `Ingest` | 3 адаптера = настоящий шов |
| Сигналы | одна append-only таблица | не восстанавливаются |
| Рекомендатель | шов `trait Distance`, 3 адаптера | смена модели не трогает интерфейс |
| ↺ Воспроизведение | свой движок: gapless, кроссфейд, beat-match, стемы | ради этого пишется сервис, Media3 этого не умеет |
| Авторизация | `Tailscale-User-Login` + allowlist | пароль — вторая копия проверки, которую уже сделал Tailscale |
| MCP | `rmcp` 3.5.0, ~10 инструментов | адаптер на том же шве, что HTTP |
| ↺ Android | Flutter + media_kit (mpv) + audio_service | нужен ещё Linux и macOS: одна кодовая база вместо трёх |
| ↺ Палитра | не нужна: дизайн монохромный (DESIGN.md), цвет берётся только из обложки | Material 3 больше нет |

---

## Глоссарий домена

Термины, которые использует код. Не изобретать новые.

| Термин | Значение |
|---|---|
| **Master** | файл на диске как есть. Никогда не мутируется. Цель дедупликации |
| **Track** | музыкальная единица каталога. Может иметь ноль, один или несколько Master |
| **Render** | претранскодированный производный файл: `opus96`, `opus128`, `flac` |
| **Library** | весь каталог. Одна на всех, доступ разграничивается по User |
| **Ingest** | путь от файла/потока до записи в каталог |
| **PlayEvent** | одна строка журнала. Append-only |
| **Session** | производная величина: последовательность событий с перерывом < 30 мин |
| **Taste** | производная величина: что пользователь любит, в виде вектора + весов |
| **Distance** | мера похожести двух треков. Реализуется тремя адаптерами |
| **SignalLog** | append-only журнал PlayEvent + производные представления |
| **Allowlist** | таблица `user`. Регистрация = добавить строку |

---

## Карта модулей и швов

```
        HTTP-адаптер ─────┐
                          │
        MCP-адаптер ──────┼──→  MusicService          ← домен, чистая логика
                          │       (единственный интерфейс)
        Android-клиент ───┘             │
                                        ├──→  LibraryRepo   →  SQLite
                                        ├──→  SignalLog     →  SQLite (append-only)
                                        ├──→  Recommend      →  Distance
                                        └──→  Palette        →  SQLite

        Ingest ──→ Transcoder ──→ Catalog
           ▲
     ┌─────┼──────────────┐
 WatchFolder  WebUpload  ApiPull        ← 3 адаптера, отдают только
                                           (bytes, filename_hint, source)

        Distance
           ▲
     ┌─────┼──────────────┐
 DspCosine  BlissLearned  BurnNn        ← 3 адаптера
```

**Швы:** `MusicService` (домен ↔ транспорт), `Ingest` (источники ↔ каталог),
`Distance` (рекомендатель ↔ модель). Три адаптера на каждом — шов настоящий, а не
гипотетический.

**Поверхностным остаётся** `Stream` (раздача файла) и `Whoami` (одна функция).
Это осознанно, см. «Не делаем».

---

## Схема данных

```sql
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

-- Allowlist. Это же таблица пользователей. Паролей нет, сессий нет.
CREATE TABLE user (
  id         INTEGER PRIMARY KEY,
  ts_login   TEXT NOT NULL UNIQUE,   -- значение заголовка Tailscale-User-Login
  name       TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

-- Файл на диске. Никогда не мутируется. Цель дедупликации.
CREATE TABLE master (
  id          INTEGER PRIMARY KEY,
  path        TEXT NOT NULL UNIQUE,
  bytes       INTEGER NOT NULL,
  mtime       INTEGER NOT NULL,
  codec       TEXT NOT NULL,          -- flac | wav | mp3 | opus
  duration_ms INTEGER NOT NULL,
  sample_rate INTEGER NOT NULL,
  channels    INTEGER NOT NULL,
  bits        INTEGER,
  tag_raw     TEXT,                   -- JSON целиком, что вернул ffprobe
  ingested_at INTEGER NOT NULL
);

CREATE TABLE artist (
  id    INTEGER PRIMARY KEY,
  name  TEXT NOT NULL,
  norm  TEXT NOT NULL UNIQUE          -- lowercase, без схлопнутых пробелов
);

CREATE TABLE album (
  id           INTEGER PRIMARY KEY,
  title        TEXT NOT NULL,
  artist_id    INTEGER NOT NULL REFERENCES artist(id),
  disc_total   INTEGER,
  norm         TEXT NOT NULL UNIQUE   -- artist + '/' + lowercase title
);

-- Музыкальная единица. Может быть без Master (ещё не отрисована).
CREATE TABLE track (
  id          INTEGER PRIMARY KEY,
  title       TEXT NOT NULL,
  album_id    INTEGER NOT NULL REFERENCES album(id),
  disc        INTEGER,
  track_no    INTEGER,
  duration_ms INTEGER NOT NULL,
  loudness_db REAL,                   -- для нормализации громкости в клиенте
  created_at  INTEGER NOT NULL
);

-- Сколько Master'ов сошлись в один Track. Здесь же дедупликация.
CREATE TABLE track_master (
  track_id  INTEGER NOT NULL REFERENCES track(id),
  master_id INTEGER NOT NULL REFERENCES master(id),
  role      TEXT NOT NULL,            -- primary | alt
  PRIMARY KEY (track_id, master_id)
);

-- Производный файл. Транскод происходит ровно один раз, здесь.
CREATE TABLE render (
  id          INTEGER PRIMARY KEY,
  track_id    INTEGER NOT NULL REFERENCES track(id),
  quality     TEXT NOT NULL,          -- opus96 | opus128 | flac
  path        TEXT NOT NULL UNIQUE,
  bytes       INTEGER NOT NULL,
  duration_ms INTEGER NOT NULL,
  created_at  INTEGER NOT NULL,
  UNIQUE (track_id, quality)
);

-- Material 3 роли из обложки. Считаются в Rust при ингесте, один раз.
CREATE TABLE palette (
  album_id            INTEGER PRIMARY KEY REFERENCES album(id),
  model               TEXT NOT NULL,  -- mcc-v1
  primary             TEXT NOT NULL,
  primary_container   TEXT NOT NULL,
  secondary           TEXT NOT NULL,
  secondary_container TEXT NOT NULL,
  tertiary            TEXT NOT NULL,
  tertiary_container  TEXT NOT NULL,
  surface             TEXT NOT NULL,
  surface_container   TEXT NOT NULL,
  on_surface          TEXT NOT NULL
);

-- 20k треков x 128 x f32 = 9.8 MiB. Косинус считается в Rust за миллисекунды.
-- ponytail: брутфорс, векторного индекса нет. Потолок ~100k треков.
-- Апгрейд: HNSW, если пройдём 100k.
CREATE TABLE embed (
  model    TEXT NOT NULL,             -- dsp-cosine-v1 | bliss-v1 | burn-nn-v1
  track_id INTEGER NOT NULL REFERENCES track(id),
  dim      INTEGER NOT NULL,
  vec      BLOB NOT NULL,             -- dim * f32 little-endian
  PRIMARY KEY (model, track_id)
);

-- ЖУРНАЛ. Только INSERT.
-- ponytail: append-only, без UPDATE, без дедупликации. Состояние
-- пересобирается производными представлениями. Потолок: агрегация только
-- офлайн, пересчёт O(событий) на каждое чтение. Апгрейд: материализованное
-- представление (user, день), когда чтения станут медленнее чтения трека.
CREATE TABLE play_event (
  id            INTEGER PRIMARY KEY,
  ts            INTEGER NOT NULL,
  user_id       INTEGER NOT NULL REFERENCES user(id),
  track_id      INTEGER NOT NULL REFERENCES track(id),
  session_id    INTEGER NOT NULL,
  event         INTEGER NOT NULL,    -- 0 Started 1 Progressed 2 Skipped 3 Completed 4 Repeated
  position_ms   INTEGER NOT NULL,
  position_ratio REAL NOT NULL,
  source        INTEGER NOT NULL,    -- 0 Search 1 Album 2 Artist 3 Playlist 4 Radio 5 Queue 6 Mcp 7 Api
  prev_track_id INTEGER REFERENCES track(id),
  device        INTEGER NOT NULL,    -- 0 Phone 1 Speaker 2 Auto 3 Desktop
  offline       INTEGER NOT NULL,
  quality       INTEGER NOT NULL,
  client_id     TEXT
);
CREATE INDEX play_event_user_ts ON play_event (user_id, ts);
CREATE INDEX play_event_track   ON play_event (track_id);
CREATE INDEX play_event_session ON play_event (session_id);
```

---

## PlayEvent и семантика сигналов

Правила определения события, которые живут в клиенте, и веса, которые живут
в рекомендателе. Разделение осознанное: клиент знает про позицию, рекомендатель
— про вес.

| Правило | Условие | Событие |
|---|---|---|
| старт | переход на трек | `Started` |
| прогресс | каждые 15 с во время игры | `Progressed` |
| дослушал | достиг конца | `Completed` |
| скип | seek или переход раньше 20% длительности, либо раньше 20 с | `Skipped` |
| репит | нажатие повтора до 50% трека | `Repeated` |

| Событие | `source` | Вес | Комментарий |
|---|---|---|---|
| `Completed` после явного выбора | Search, Album, Artist, Playlist, Queue | **++** | и выбор, и результат |
| `Repeated` | любой | **+++** | сильнее play count |
| `Completed` из радио | Radio | **+** | приятно, но не выбирал |
| `Skipped` < 10% | Radio | **−−** | активно против |
| `Skipped` трек 3 из альбома | Album | **~0** | может не нравиться альбом, а не трек |
| `Started` и тишина 30 с | любой | **−** | поставил и ушёл |
| `Completed` офлайн | любой | **+** | чистый интерес |

Ключевое различие, которое наивная реализация упускает: **выбрал или попалось
само**. Оно целиком определяется полем `source`.

**Не собираем** то, что Spotify собирает для лейблов и рекламы: локальные файлы
по IP, рекламные ID, социальный граф. У нас нет лейблов.

---

## Приоритет метаданных в Ingest

Наблюдаемая реальность библиотеки: теги сломаны. У одного FLAC
`ARTIST: "MUSIC Engine"`, `album_artist: "Kota Kawai"`, при этом
`COMPOSER: "Toby Fox"` — настоящий артист. Ключи вперемешку: `ARTIST`, `ALBUM`,
`DATE` в верхнем регистре и `album_artist`, `disc` в нижнем. У MP3 тегов нет
вообще, всё в имени файла. Структура папок `Artist - Album [FORMAT]/`, не
Picard.

Порядок разрешения, от высшего к низшему:

1. **Встроенный тег** — после нормализации регистра и отбраковки мусора.
2. **Имя файла** `Artist - Title.flac` — если у файла тегов нет вовсе.
3. **Родительская папка** `Artist - Album [FORMAT]/` — если имя файла не даёт
   артиста.
4. **Пусто**, `Unknown Artist` — и трек помечается на ручную правку.

Правила отбраковки мусорных значений:

| Значение | Действие |
|---|---|
| `MUSIC Engine`, `Lavf*`, `Lavc*`, `Unknown`, пустая строка | отбросить, падать на следующий уровень |
| `COMPOSER` содержит реального артиста, `ARTIST` — нет | использовать `COMPOSER` как `artist` |
| значение > 300 символов | обрезать, пометить подозрительным |
| регистр ключа смешанный | привести к нижнему перед чтением |

Allowlist папок обязателен: **2.5 GiB из 4.8 — Kontakt `.nkx`/`.nka`**
(8Dio, Heavyocity). Это сэмплы для продакшена, не музыка. Явный список
разрешённых альбомов, без эвристики по имени папки.

Оригиналы **не переписываются**. Picard не установлен, бэкап единственной копии
— неоправданный риск. Нормализация живёт только в индексе.

---

## Kotlin / Rust в приложении

| В Rust (общий `.so`) | В Kotlin |
|---|---|
| `recommend`, эмбеддинги — тот же код, что на сервере | Compose UI, Material 3 |
| download-менеджер: retry, backoff, Range-resume, ETag, cache invalidation | Media3 + MediaSession |
| сверка оффлайн-кэша с сервером | нотификации, Android Auto |
| батчинг и кодирование `play_event` | WorkManager, a11y, TalkBack |

Мост: `mozilla/rust-android-gradle` (Apache-2.0, поддерживается Mozilla, последний
коммит 2026-09-24) + `cargo-ndk` + JNI, `extern "C"` с unsafe-мостом.

**Палитра M3** приезжает с сервера готовыми токенами из таблицы `palette`.
Генерация — порт алгоритма `material-color-utilities` в Rust, выполняется на
сервере один раз при ингесте. Клиент алгоритма не знает.

---

## Срезы

Каждый срез проверяем по образцу: **поставить и посмотреть глазами**, не «компилируется».

### Срез 0 — тулчейн

Установить, ничего не удаляя.

```bash
# rustup поверх системного rust, пакет rust НЕ трогаем
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup target add aarch64-linux-android armv7-linux-androideabi \
                      x86_64-linux-android i686-linux-android
rustup component add rust-src
cargo install cargo-ndk

# JDK 21 для AGP (установлен 26, он не документирован AGP)
sudo pacman -S jdk21-openjdk

# tempo/beat
sudo pacman -S aubio

# git
git config --global user.name  "..."
git config --global user.email "..."
```

Android SDK ставится через Android Studio или `cmdline-tools` (platform 36,
build-tools 36.0.0).

**Готово когда:** `cargo build` проходит; `cargo build --target aarch64-linux-android`
проходит; `java -version` печатает 21; `adb devices` показывает Pixel 9;
`pacman -Q aubio` печатает версию; `git config user.name` не пуст.

**Не делаем:** CI, деплой, домен.

---

### Срез 1 — Ingest: папка + каталог + палитра

Один адаптер — watch folder. Без транскода.

**Готово когда:** FLAC, сваленный в `incoming/`, появляется в индексе с осмысленным
артистом; MP3 без единого тега получает артиста из имени файла; трек с
`ARTIST: "MUSIC Engine"` получает артиста из `COMPOSER`; папки Kontakt
игнорируются; `GET /albums` возвращает альбом с непустой `palette`.

**Не делаем:** web upload, API, транскод, воспроизведение.

---

### Срез 2 — Render и раздача

```bash
ffmpeg -i <master> -c:a libopus -b:a 96k -vbr on -f opus <render>
```

**Готово когда:** после рестарта у всех треков есть `render`; `GET /track/:id/stream`
отдаёт `206` с корректным `Content-Range`; `ffprobe` на render показывает
`codec_name=opus` в контейнере `ogg`; seek на середину трека с телефона не
перематывает (проверяется вручную); ингест 20k файлов переживает рестарт.

**Не делаем:** кэш, очередь, докачка, gapless.

---

### Срез 3 — SignalLog ← срез с lead time

Схема, приёмник, производные представления. Рекомендателя ещё нет.

**Готово когда:** `POST /events` с задокументированным `PlayEvent` возвращает 201;
строка в базе побайтово равна отправленной; повторный идентичный POST создаёт
вторую строку, а не перезаписывает; представление `v_track_affinity` на
трёхсобытийной фикстуре ранжирует B выше C; `git log` показывает, что схема
закоммичена до появления Recommend; все пять потребителей (HTTP, MCP, Android,
Recommend, DSP) ссылаются на одну схему и ни один не потребовал изменения
собственного дизайна.

**Не делаем:** рекомендатель, UI, любые выводы из данных.

---

### Срез 4 — Whoami

**Готово когда:** запрос без `Tailscale-User-Login` получает 401; с логином вне
allowlist — 403; с правильным — 200 от своего пользователя; сервер слушает
только 127.0.0.1; `tailscale serve` открывает доступ с Pixel 9.

**Не делаем:** пароли, сессии, RBAC, регистрация.

---

### Срез 5 — Android v0 ← **часы здесь пошли**

Минимальный клиент: список альбомов, тап, звук, уведомление, пауза, отчёт.

**Готово когда:** приложение на Pixel 9 показывает альбомы с сервера через
Tailscale; тап по треку играет Ogg-Opus через динамик; уведомление появляется и
ставит на паузу; через 15 с в `play_event` появляется `Progressed` с
`device=Phone`; скип фиксируется как `Skipped`.

**Не делаем:** оффлайн, очередь, плейлисты, динамические темы, Android Auto.

**С этого момента данные накапливаются.** Всё дальнейшее можно делать в любом
темпе — часы уже идут.

---

### Срез 6 — Recommend v1 + MCP

DSP-адаптер шва `Distance`: `rustfft`/`realfft` → mel → chroma → tempo, косинус.

**Готово когда:** `GET /recommend?limit=10` возвращает 10 id; для фикстуры
«слушал A→B, скипнул C» B ранжируется выше C; холодный юзер получает
детерминированный fallback, а не пустой список; ранжирование 20k треков
занимает меньше 50 мс; MCP-клиент видит 10 инструментов; `search` находит
известный трек; `now_playing` отражает реальную игру; команда `play` из MCP
попадает в `play_event` с `source=Mcp`.

**Не делаем:** обучение, GPU, NN.

---

### Срез 7 — Оффлайн и очередь

**Готово когда:** трек скачивается и играет в авиарежиме; kill процесса
после середины загрузки — докачка продолжается, а не начинается заново;
удалённый на сервере трек исчезает из оффлайна; очередь переживает сворачивание
приложения; динамические темы из `palette` применены, включая светлую и
тёмную; Android Auto показывает очередь.

**Не делаем:** lossless-оффлайн по Wi-Fi (добавится одной настройкой позже),
плейлисты пользователей.

---

### Срез 8 — Адаптеры 2 и 3

**Готово когда:** файл, загруженный через web-интерфейс, появляется в каталоге
и идентичен по метаданным тому же файлу, положенному в папку; загрузка
100 файлов показывает прогресс и отменяется на середине без записи в каталог;
`ApiPull` забирает трек с разрешённой лицензией и создаёт те же строки, что и
watch folder.

**Не делаем:** источники без явной лицензии, дедупликация на уровне API.

---

### Срез 9 — Burn NN

Третий адаптер `Distance`. Первые два продолжают работать.

**Готово когда:** `Distance` собирается с каждым из трёх адаптеров по отдельности;
`burn-nn-v1` даёт эмбеддинги, коррелирующие с `dsp-cosine-v1` выше
случайного уровня на эталонном наборе; переключение адаптера не меняет ни одного
файла вне `recommend/`.

**Не делаем:** онлайн-обучение, деплой модели, GPU-инференс в приложении.

---

## Не делаем

| Не строим | Почему |
|---|---|
| Движок воспроизведения (очередь, crossfade, replaygain, gapless) | Media3 делает это, включая MediaSession, который обязателен для Android Auto |
| Транскод на лету | претранскод делает seek точным и бесплатным; на лету — это кэш, инвалидация и очередь |
| Пароли, сессии, RBAC, регистрация | 2 человека в tailnet; `Tailscale-User-Login` уже проверен и не подделывается |
| Векторная БД | 9.8 MiB эмбеддингов; брутфорс — миллисекунды |
| Перезапись тегов на диске | бэкап единственной копии — неоправданный риск |
| PICARD-подобный таггер в приложении | ингест уже нормализует; второй таггер = две правды |
| iOS | AVPlayer не декодирует Opus |
| Онлайн-обучение | 2 пользователя; данных не будет никогда достаточно |
| GPU-инференс в приложении | исследовательская задача, в профиль не входит |

---

## Риски

| Риск | Вероятность | Что делаем |
|---|---|---|
| Burn pre-1.0 ломает API (0.21 → 0.22 уже ломает чекпоинты) | высокая | весь NN за `trait Distance`; адаптеры 1-2 работают без Burn вообще |
| burn#5122: fusion даёт мусор на sm_120 | высокая | fusion выключить, CUDA backend, не wgpu; не-фьюженный `CubeBackend<CudaRuntime>` корректен |
| Теги хуже, чем в выборке | средняя | приоритет из 4 уровней + отбраковка; всё падает до `Unknown Artist` и чинится |
| aubio / tempo — самая долгая фича | высокая | `pacman -S aubio` в срезе 0; fallback: темп исключается из v1, эмбеддинги на mel+chroma |
| Свежая коробка, Android-тулчейн с нуля | высокая | срез 0 существует именно для этого и проверяется явно |
| Качество Opus ниже ожиданий | средняя | 96k — прозрачно для большинства; 48k уже артефакты; lossless-фоллбэк — одна настройка |
| Библиотека вырастет до 20k+ | средняя | брутфорс остаётся в силе до 100k; эмбеддинги — 9.8 MiB |
| Pixel 9 не в tailnet при первом запуске | средняя | `tailscale up` на устройстве — шаг среза 0 |
| MCP-клиент не подключается к Rust-серверу | низкая | `rmcp` 3.5.0 на стабильной спецификации, 3 слоя транспорта |

---

## Открытые вопросы

Не блокеры. Зафиксированы как допущения, вернуться при первом проявлении.

- **Источник free-licensed музыки.** Jameno (есть API, фильтр CC0/CC-BY),
  Free Music Archive, ccMixter или вручную. Не влияет на интерфейс `Ingest`:
  все три адаптера отдают `(bytes, filename_hint, source)`. Определится при
  срезе 8.
- **Общий аккаунт семьи.** Сейчас каждый пользователь taste видит свой. Объединённый
  вкус — отдельная строка в `user` с флагом. Не мешает.
- **Lossless по Wi-Fi.** В `render` уже есть колонка `quality`; клиенту нужен
  переключатель. Не мешает.
- **Веб-клиент.** Ответ «c» на раннем вопросе, не обсуждался. Вне v1.
- **Нормализация громкости.** `loudness_db` в схеме есть, вычисление — нет.
  Добавить, когда первая жалоба на «один трек громче другого».

---

## Память

- Burn на sm_120: fusion выключать, CUDA backend, wgpu не использовать —
  burn#5122 (мусор на wgpu, неверные числа на fused CUDA) и wgpu#10484 (ReBAR
  рвёт скорость загрузки). Обе вины не в железе.
- Arch `rust` в `/usr/bin` и `rustup` в `~/.rustup` + `~/.cargo/bin` не конфликтуют.
  Пакет `rust` не удалять. `~/.cargo/bin` уже первый в PATH.
- Теги: индекс, не файлы. Оригинал — только чтение, всегда.
- Ogg-Opus (`-f opus`), не «просто opus»: автоопределение по расширению `.opus`
  в Media3 не документировано, а `OggExtractor` даёт настоящий seek.
- AGP 9.4.0 документирует минимум JDK 17; 21 — безопасная пара, 26 — нет.


---

## Ревизия 2026-11-01

| Область | Было (28.09) | Стало |
|---|---|---|
| Клиент | Kotlin + Compose + Media3, Android only | Flutter + media_kit: Android, Linux, macOS |
| Движок | «не строим» | свой: gapless, кроссфейд, beat-match, стемы (Demucs) |
| Дизайн | Material 3 + палитры | монохром, см. `DESIGN.md` |
| Транскод | претранскод всех в opus96/128 | FLAC-мастер + Opus 128 по требованию |
| Библиотека | free-licensed, 2–3 юзера | личная библиотека + импорт по ссылке |
| Носитель | не оговорён | FLAC на HDD (574 ГБ свободно), БД и кэш на NVMe |

**Остаётся в силе:** `master / track / render`; `play_event` с первого дня; приоритет
метаданных и отбраковка мусора; Tailscale + allowlist; `trait Distance`; срезы с
проверкой глазами; теги на диске не мутируются.

**Статус на 2026-11-01:** `server/` — axum + SQLite, инкрементальный скан
(`artist`/`album`/`track`/`master`/`track_master`), FTS5, Range-стриминг 206/416,
`POST /api/events` с allowlist, `GET /api/manifest?since=rev`. 20 тестов, все зелёные.

---

## Срез рекомендаций (2026-11-01)

Сделано:
- `features.rs` — DSP-эмбеддинг `dsp-v1`, 56 измерений (40 log-mel, 12 chroma,
  centroid/flatness/rolloff, темп), L2, косинус. Тесты: питч-класс, норма, порядок
  сходства, блоб, тишина.
- `POST /api/features/build?limit=N` — инкрементальный расчёт, вызывается клиентом
  после скана.
- `GET /api/recs/similar/{id}?n=` — брутфорс-косинус по всем эмбеддингам
  (10k × 56 float ≈ 5 мс), сид досчитывается на месте.
- Клиент: правая кнопка на треке → «радио от этого трека», вкладка `radio` показывает
  очередь; кнопка пересканирования на главной.

Следующее: прототипы вкуса, бандит, ежедневные миксы; затем нейросетевой эмбеддер
(CLAP/MuQ) за тем же швом `Distance` с A/B по held-out.
