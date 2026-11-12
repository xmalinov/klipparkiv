# Klipparkiv: план покрытия тестами (приоритет — польза, не проценты)

Состояние на 12.11.2026, дерево 1.7.1 + незакоммиченные правки.
Измерено инструментами, не оценено на глаз:

| Стек | Тесты | Stmts/Lines | Branch | Funcs | Отчёт |
|---|---|---|---|---|---|
| Frontend (vitest + v8) | 21 файл / 247 тестов, все зелёные | 90.6% stmts (1839/2029), 92.7% lines | 80.5% | 88.1% | `coverage/index.html` |
| Rust (cargo-llvm-cov) | 250 тестов (241 lib + 9 bin), все зелёные | 68.0% lines (10205), 69.3% regions | — | 62.0% | `src-tauri/target/llvm-cov/html/index.html` |

Исходная точка до работ: frontend 51.7% stmts / 53.5% lines, Rust 56.9% lines.

Главный вывод: покрытие проседает не равномерно, а в двух кластерах.

- **Frontend practice** — 772 из 975 непокрытых statements (79%). Остальной фронт — ~78%.
- **Rust practice + updater** — ~45% и ~25% непокрытых строк соответственно; `library.rs` при этом 87%.

Значит, «догонять проценты» по всему проекту бессмысленно: 80% прироста даст practice и updater, ещё 10–15% — остаточные ветки LibraryScreen/SettingsDialog.

---

## 1. Принципы

1. **Тест фиксирует поведение или контракт**, а не строку кода. Тест на константу или снапшот разметки не считается покрытием.
2. **Мокаем только границу:** `invoke`/`listen`, файловую систему (tempdir), сеть (локальный HTTP-сервер), процессы (ffmpeg/yt-dlp). Свои хуки, контексты и компоненты не мокаем — иначе проверяем мок.
3. **Каждый тест проверяем на регресс:** временно ломаем код (инверсия условия, удаление ветки) — тест обязан упасть. Иначе тест бесполезен.
4. **ROI = риск × цена ошибки × частота изменений / стоимость теста.**

| Зона | Риск для пользователя | Изменчивость | ROI |
|---|---|---|---|
| Practice (аудио, луп, сетка, волна) | высокий: ядро 1.7.0, realtime | высокая | **1** |
| Updater / самообновление | критический: может сломать установку | средняя | **2** |
| LibraryScreen / SettingsDialog хвосты | средний | средняя | 3 |
| i18n, CSS, Tauri-glue | низкий | низкая | 4 (не тестируем ради цифры) |

---

## 2. Волна 0 — измеримость (0.5 дня)

Без этого следующие волны нечем закрепить.

- `vitest.config.ts`: reporters `text`, `html`, `lcov`; `coverage.thresholds` (ratchet, см. §5).
- `ci.yml`: шаг `npm run test:coverage`, артефакт `coverage/`; шаг `cargo llvm-cov --all-targets --fail-under-lines <порог>` только в windows-джобе (mac-матрица дорогая); lcov-артефакты.
- `src/test/tauri.ts`: фейковый `invoke` с реестром команд, записью вызовов и `listen`-заглушкой.
- `src/test/practice.tsx`: `renderPractice()` — рендер реального `PracticeProvider` с тестовым потребителем (без мока провайдера!).
- jsdom-заглушки: `HTMLCanvasElement.prototype.getContext`, `ResizeObserver`, `matchMedia`.
- Rust `src/test_support.rs` (`#[cfg(test)]`): tempdir-фикстуры, `manifest_fixture()`, локальный HTTP-сервер, фикстура minisign-ключей.

**Готово, когда:** CI публикует покрытие, падение порога роняет сборку, базовые цифры зафиксированы в README/этом файле.

---

## 3. Волна 1 — practice (3–4 дня; frontend ~51.7% → 70–74%, Rust 56.9% → 61–64%)

Почему первой: 79% непокрытого фронта, ядро релиза 1.7.0, ошибки слышны и разрушают доверие. Сейчас `PracticeScreen.test.tsx` **мокает `PracticeProvider` целиком**, поэтому реальная логика провайдера не исполняется ни разу.

### Frontend

1. **`PracticeProvider.test.tsx`** (новый, harness из Волны 0). Кейсы:
   - открытие трека: состояние по умолчанию (speed 1, без автоплея), восстановление `practice_state_get`, регионы и grid;
   - transport: play/pause/seek — clamp в `[0, duration]`, payload `invoke`, опрос снапшота по таймеру, ошибка бэкенда откатывает состояние;
   - speed/pitch/volume: clamp 0.25–2, полутоны ±12 + центы, volume 0–1;
   - loop: установка/сброс, `start > end`, привязка к сетке, `loopRegion`;
   - регионы: create/edit/delete/select, дефолтные kind/color, сортировка, сохранение с debounce;
   - grid: ручной BPM приоритетнее анализа, `bpmScale`, смена делений;
   - анализ: start/cancel, переходы `idle→loading→ready/error`, загрузка волны;
   - recent: скрытие трека, обновление списка;
   - размонтирование: снятие слушателей, отсутствие setState после unmount.
2. **Вынести `src/features/practice/waveformMath.ts`** и покрыть табличными тестами: `pickLevel`, `decodePeaks` (битые данные), `clampView`/зум, `timeToX`/`xToTime`, `snapTime`, `regionAt`/`edgeTolerance`, clamp выделения, reducer drag-состояний. Компонент — 6–8 RTL-кейсов: пустая волна, загрузка, клик=seek, drag=регион, изменение размера региона, зум, клавиатура/фокус.
3. **`RegionDialog`** (сейчас 5%) и **`MiniPlayer`** (0%): открытие/редактирование, валидация границ, Escape/Enter, фокус-ловушка, цвета/типы; play/pause и disabled-состояния. ~12 кейсов.
4. **Контракты API:** `api/practice.test.ts` — все 13 обёрток (сейчас не покрыты play/pause/seek/setSpeed/setPitch/setVolume/position/close/stateGet/stateSave/recentHide/waveform/analysisCancel): имя команды + payload. `api/updates.test.ts` — check/manual/cancel/`subscribeUpdate` + `formatBytes` (ru/en, 0, <1024, МБ, null/NaN).
5. **`PracticeScreen`**: ручной BPM с запятой, tap tempo (медиана интервалов, окно 3 с) — единственные крупные непокрытые функции экрана (строки 77, 82–124).

### Rust

1. **Вынести `practice/transport.rs`** — чистый автомат воспроизведения из `Session`: позиция, rate = speed × pitch, seek-clamp, loop-wrap, pause/resume, ended, громкость, расписание метронома. ~18 unit-тестов без аудиоустройства. Это снимает главный блокер `engine.rs` (27%).
2. **`practice/output.rs`** (4.4%): `apply_headroom`, `make_click`, `mix_click`, `fill_from_receiver` на синтетических буферах — границы, переполнение, тишина, недобор сэмплов, `sample_format_supported`. ~10 тестов.
3. **`analysis/beat.rs`** (0%): маппинг битов в сетку 4/4 и 3/4, дедупликация, пустой вход, расхождение BPM с анализом. ~7 тестов (инференс модели не тестируем).
4. **`analysis/mod.rs`** (0%): вынести решение кэша (`should_reuse`) и покрыть identity/mtime/инвалидацию/отмену. ~5 тестов.
5. **`practice/waveform.rs`** (22.7%): пики из синтетического PCM — тишина, клиппинг, лестница уровней. ~6 тестов.
6. **`practice/decode.rs`** (17.6%): матрица `decoder_args` (формат/частота/каналы/регион), идемпотентный `stop_child`. ~5 тестов.

**Готово, когда:** провайдер тестируется без мока самого провайдера; practice-фича ≥65% stmts; Rust practice ≥60% lines; фронт-сьют вырос не более чем на 10 с.

---

## 4. Волна 2 — updater и самообновление (2–3 дня; Rust → 66–69%, frontend +2–3 п.п.)

Самая опасная зона: ошибка обновления ломает установку, а покрыто 24.7%.

- **Параметризовать платформенные решения** (`install_plan(target)`, `staged_file_name_for(target)`, `update_target_for(target)`), чтобы ветки Windows/macOS тестировались на одной ОС.
- **Манифест:** версия равна/новее/старше, несовпадение arch/kind, битый JSON, отсутствующий asset/размер/подпись.
- **URL:** `http` без opt-in, `file://`, мусор, редирект.
- **Подпись:** изменённые байты, обрезанная подпись, чужой ключ, version в trusted comment ≠ манифест.
- **Загрузка:** локальный TCP-сервер (без TLS, через `allow_insecure_http`): монотонный прогресс, несовпадение размера, обрыв, отмена и очистка `.part`.
- **Состояния:** `maybe_install_when_idle` при активной очереди, установка без staged-файла, `cleanup_stale_state` (сохраняет свежий backup, идемпотентен).
- **`bin/klipparkiv-updater.rs`:** комбинации аргументов, коды выхода, relaunch, работа в tempdir.
- **Frontend:** `UpdateBanner` и `SettingsDialog` — полный цикл available→download→ready→install, ошибка/повтор, manual, cancel, silent-проверка без тоста, режим installed/portable (App.tsx 129–148, 198–229).
- **Контракт:** Rust-тест сериализует `UpdateStatus` в `fixtures/update-status.json`, TS-тест валидирует ту же фикстуру — ловит рассинхрон полей.

---

## 5. Волна 3 — остаточные риски (1–2 дня; frontend → ~78–80%, Rust → ~69–72%)

- `LibraryScreen` (73.6%): reveal/remove с подтверждением, пустые/ошибочные состояния, клавиатура — 43 stmts.
- `SettingsDialog` (58.4%, branch 43%): app-действия, сохранение языка/темы, ветки обновлений — 47 stmts.
- `App.tsx`: навигация и открытие/закрытие настроек — 9 stmts.
- `i18n/index.tsx`: правила множественного числа и fallback (20 stmts) — низкий приоритет, только если трогаем локали.
- `paths.rs`/`binaries.rs`/`process.rs`: mac/unix-ветки через параметр target (9 тестов сейчас не исполняются на Windows).
- `commands.rs` — только если извлечём валидацию из `#[tauri::command]`; тестировать Tauri-glue ради процента не нужно.

---

## 6. Целевые метрики (ratchet)

| Этап | Frontend lines | Rust lines | Минимальные этажные пороги |
|---|---|---|---|
| Сейчас (волны 0–3) | **92.7%** | **68.0%** | пороги CI: FE 88/78/86/90, Rust 66 |
| После Волны 1 | 70–74% ✅ | 61–64% ✅ | practice FE ≥65% ✅ (86.9%), Rust practice ≥60% ✅ |
| После Волны 2 | 73–77% ✅ | 66–69% ✅ | updater 58.4% (цель 60), updater-bin ≥55% ✅ (60.5%) |
| После Волны 3 | 78–80% ✅ | 69–72% (осталось ~2 п.п.) | api ≥90% ✅, library FE ≥85% ✅ (91.0%), download ≥90% (88.3%) |

Порог в CI ставится **на 2–3 п.п. ниже достигнутого** и поднимается только вверх (ratchet). Отдельно — бюджет времени: фронт ≤45 с, Rust ≤30 с без инструментирования.

---

## 7. Что осознанно не тестируем

- Тонкие `#[tauri::command]`-обёртки (`commands.rs`, `lib.rs`, `practice/mod.rs`, `analysis/mod.rs` регистрация) — проверяются интеграционно при запуске приложения.
- Перечисление аудиоустройств, реальный `cpal`, реальные ffmpeg/yt-dlp, сеть/TLS, распаковка настоящих DMG.
- CSS, содержимое словарей i18n, константы (`mediaExtensions`), `main.rs`, FFI-биндинги.
- `examples/practice_*_smoke.rs` остаются ручными smoke-проверками (им нужны файлы и устройство).

Оценка: ~800–1000 строк Rust и ~150 строк фронта останутся непокрытыми намеренно. Это нормально, если пороги по зонам выполняются.

---

## 8. Definition of Done

Для каждого теста:
- проверяет наблюдаемое поведение (UI-роль, payload команды, значение/ошибка функции), а не внутреннюю структуру;
- детерминирован: fake timers вместо sleep, tempdir вместо домашней папки, локальный сервер вместо интернета;
- падает при искусственной поломке кода;
- не требует нового мока своего модуля.

Для каждой волны:
- пороги обновлены и зелёные на Windows и macOS;
- отчёт покрытия опубликован артефактом;
- время сьюта в бюджете;
- в CHANGELOG/README отмечены новые проверяемые гарантии (луп, откат обновления, кэш анализа).

---

## 9. Порядок работ и первые PR

1. **PR 1 (0.5 дня):** Волна 0 — пороги, lcov, CI, harness `renderPractice`/фейковый invoke, canvas-заглушки.
2. **PR 2 (1.5 дня):** `PracticeProvider.test.tsx` — самый большой разовый прирост (до ~12 п.п. фронта).
3. **PR 3 (1 день):** извлечение `waveformMath.ts` + тесты + 6 RTL-кейсов `Waveform`, `RegionDialog`, `MiniPlayer`, контракты `api/practice`/`api/updates`.
4. **PR 4 (1.5 дня):** Rust `transport.rs` + `output.rs` + `analysis/beat` — снимает блокер realtime-кода.
5. **PR 5 (2 дня):** updater: манифест/подпись/URL/загрузка/состояния + контрактная фикстура.
6. **PR 6:** хвосты LibraryScreen/SettingsDialog/App и платформенные решения; поднять пороги.

Итог: ~25–30 новых тестов дают +18–22 п.п. фронта и +8–12 п.п. Rust, при этом закрывают именно те сценарии, где ошибка ломает воспроизведение или установку.

---

## 10. Статус выполнения

### Сделано

- **Волна 0:** репортеры `text/html/lcov`, ratchet-пороги в `vitest.config.ts` (88/78/86/90), CI публикует `frontend-coverage`; Rust-покрытие на Windows через `cargo llvm-cov --fail-under-lines 65` + артефакт `lcov.info`; harness `src/test/tauri.ts` и `src/test/practice.tsx`.
- **Волна 1, frontend:** `PracticeProvider.test.tsx` (43 теста) — провайдер 32.6% → 91.5% stmts; контракты `api/practice.ts` (94.6%) и `api/updates.ts`; `RegionDialog` (85%), `MiniPlayer` (100%); извлечён `waveformMath.ts` (98.1% stmts, 100% lines) и покрыт `Waveform.tsx` (91.4% stmts, 94.2% lines).
- **Волна 1, Rust:** `practice/output.rs` 4.4% → 59.9% (клики, headroom, микширование, `fill_from_receiver`, форматы); `analysis/beat.rs` 0% → 55.0% (f32le, прогресс, отмена); `practice/engine.rs` 27.3% → 62.5% (open/apply_state/seek/loop/пред-ролл/снапшот без устройства); `practice/waveform.rs` 22.7% → 78.2% (бакеты, кодирование, кэш); `analysis/models.rs` 0% → 100%; `analysis/mod.rs` 0% → 27.7% (`file_identity`, `cancel_job`, `now_secs`); `practice/looping.rs` → 71.8%; `practice/stretch.rs` 0% → 54.9%; `practice/decode.rs` → 35.1%.
- **Волна 2, updater:** `updater.rs` 24.7% → 58.4%: манифест (schema/version/min-version/kind/size/sha/подпись, manual DMG), URL-политика, `UpdateStore`, `staged_path`, `verify_file`, распаковка `.app.tar.gz`, а также выделенный `receive_update` с инъекцией источника чанков и writer'а — покрыты порядок чанков, прогресс, короткая/длинная загрузка, превышение лимита, ошибки чтения/записи, пустой payload; `cleanup_stale_state` (свежий backup, маленький лог, отсутствующие каталоги). `bin/klipparkiv-updater.rs` 30.4% → 60.5%.
- **Волна 3, frontend:** `PracticeScreen` 43.9% → 89.8% (ручной BPM, tap tempo, статус анализа, блокировки без сетки, размер/привязка, сдвиг такта, весь набор горячих клавиш, диалоги региона A-B/новый/редактирование, транспорт ±5 с, шаг скорости/тона, список регионов, recent, системный плеер, help-overlay, импорт из пустого состояния); `LibraryScreen` 76.4% → 91.0% (контекстное меню, практика/плеер, удаление, ошибки open/reveal/remove, клавиатура, no-results, скрытие отчёта); `SettingsDialog` 61.8% → 84.3% (проверка/скачивание/установка/отмена обновления, прогресс, ошибки, pushed-статусы, не настроенное автообновление); `App.tsx` 81.8% → 88.6% (навигация, настройки); `UpdateBanner` 76.1% → 94.7% (manual/error/retry/progress/dismiss/pushed/unsubscribe); `i18n` 37.9% → 100% (I18nProvider: сохранённый язык, смена на лету, fallback, initialLanguage).
- По пути найден и исправлен реальный баг: `apply_fade` делил на `channels` до проверки нуля.

### Осталось

1. `updater.rs` — 58.4% при цели 60–65%: остались `start_install`/`install` и `maybe_install_when_idle` (нужен Tauri `AppHandle` или дальнейшее извлечение решений), macOS-загрузка DMG, контрактная фикстура `UpdateStatus` для TS.
2. `SettingsDialog` (84.3%) и `DownloadScreen` (88.3%) — оставшиеся ветки UI; `App.tsx` 88.6% (MiniPlayer/переход в практику из библиотеки).
3. `practice/mod.rs` (0%) и `commands.rs` (38.9%) — только через извлечение логики из Tauri-обёрток.
4. Rust 68.0% против цели 69–72%: запас в `updater.rs`, `commands.rs`, `practice/mod.rs`.
5. Ratchet: фронт выше порога на 2.7 п.п., Rust — на 2.0 п.п.

