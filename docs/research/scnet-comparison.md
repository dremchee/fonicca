# Spleeter и SCNet-large, 2026-10-25

На трёх фрагментах тестовой части MUSDB18 SCNet-large улучшил SI-SDR всех
12 стемов относительно текущего Spleeter. Это основание добавить его как режим
качества, а Spleeter оставить для быстрой обработки. С VirtualDJ сравнения не было.

| Средний SI-SDR, дБ | Spleeter | SCNet-large | Разница |
|---|---:|---:|---:|
| Вокал | 8,03 | 13,10 | +5,07 |
| Ударные | 5,40 | 10,24 | +4,84 |
| Бас | 0,74 | 6,41 | +5,67 |
| Остальное | 3,47 | 7,25 | +3,77 |

Сырой отчёт: [scnet-comparison.json](scnet-comparison.json).

## Материал и метод

Официальные [7-секундные превью MUSDB18](https://github.com/sigsep/sigsep-mus-db/releases/download/v0.4.0/MUSDB18-7-STEMS.zip),
раздел `test`: Cristina Vane — So Easy, Louis Cressy Band — Good Time,
Secretariat — Borderline. Первые три тестовых файла в архиве, без отбора по результату.
Каждый декодирован FFmpeg в 44,1 кГц stereo float32: поток 0 — mix,
1 — drums, 2 — bass, 3 — other, 4 — vocals; фактическая длина 6,8034467 с.
Используются отдельные исходные стемы из того же файла, а не сумма результатов модели.

SI-SDR вычислен по стерео, последовательно развёрнутому в один вектор:
убрать среднее, спроецировать оценку на эталон и взять отношение энергии проекции
к остатку, 10 log10. Чем больше, тем лучше. Это **не BSSEval SDR/museval** и числа
нельзя напрямую сравнивать с таблицами публикаций. Итог — среднее по трём песням.
AAC в превью отличается от исходного MUSDB18-HQ; короткие фрагменты не показывают
поведение на всём треке. Метрика не заменяет прослушивание артефактов.

Spleeter: наш Rust `stems::dsp::Separator`, четыре FP16 ONNX-графа с закреплённой
ревизией. SCNet-large: ONNX с закреплённой ревизией, Python ONNX Runtime 1.31.0 и
независимая обработка STFT/iSTFT через PyTorch 2.14.1. Результат нативной Rust
реализации дополнительно сверен с Python на официальном примере Spleeter:
128–130 дБ SNR, отличие существенно ниже слышимого уровня.
Модели, аудио и виртуальное окружение не включены в Git.

## Производительность

MacBook с M1 Pro, 16 ГБ, CPU, четыре потока, debug-сборка Rust с opt-level 1
(зависимости opt-level 3). Время не является чистым сравнением библиотек:
SCNet-метрики включают загрузку и сохранение через Python, Spleeter — загрузку
и разделение в Rust. На коротких фрагментах SCNet считает полное окно в 11 с.

- Spleeter: 0,65–0,67 с на 6,80 с аудио внутри теста, около 0,74 с с запуском процесса.
- SCNet-large: 11,36–13,82 с на 6,80 с аудио в Python.
- Нативный SCNet-large на другом 8-секундном примере: 19,56 с, 2,51 ГБ пикового RSS,
  macOS peak memory footprint 3,83 ГБ. Во время замера также шла компиляция,
  поэтому это проверка работоспособности, а не оценка максимальной скорости.

Для длинных треков SCNet использует перекрытия 75% и дополнительные поля, так что
время нельзя линейно вывести из одной короткой записи. GPU/CoreML пока не включены.

## Воспроизведение нативных проверок

Из папки `src-tauri`, с отдельно скачанной моделью:

```sh
cargo test --lib stems::scnet::tests
SCNET_MODELS=/path/to/scnet-directory STEM_AUDIO=/path/to/8s.wav \
  SCNET_REFERENCE=/path/to/python-stems \
  cargo test --lib stems::scnet::tests::real_model_inference -- --ignored --exact --nocapture
SCNET_MODELS=/path/to/scnet-directory STEM_AUDIO=/path/to/12s.wav \
  cargo test --lib stems::tests::scnet_file_export_keeps_original_and_cleans_cancelled_work -- --ignored --exact --nocapture
cargo test --lib stems::models::network_tests::downloads_and_verifies_scnet_with_application_downloader -- --ignored --exact --nocapture
```

Локальная страница прослушивания с результатами, эталонами и скриптами сравнения:
`~/.codex/artifacts/fonicca-stems-2026-10-25/comparison.html`.
Материал остаётся локальным, права на песни принадлежат авторам.
