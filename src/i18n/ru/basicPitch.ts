import type en from '../en/basicPitch'
export default {
  title: 'Аудио в MIDI', action: 'Экспорт MIDI…', description: 'Распознавание нот через Spotify Basic Pitch и сохранение MIDI для вашей DAW. Аудио остаётся на этом компьютере.',
  saveTitle: 'Сохранить MIDI', sources: { file: 'Весь аудиофайл', selection: 'Выделенный фрагмент отредактированного аудио', editor: 'Всё аудио редактора, включая несохранённые изменения' },
  ready: 'Basic Pitch установлена · локальная обработка', download: 'При первом экспорте загрузится Basic Pitch ({size}).',
  hint: 'Лучше подходит для одного мелодического инструмента: баса, пианино или гитары. Сохраняет время и громкость нот; без pitch bend.',
  limits: 'Моно или стерео, до 10 минут. Ваше аудио не изменяется.',
  start: 'Экспорт MIDI…', downloadStart: 'Скачать модель и экспортировать…', close: 'Закрыть', stop: 'Остановить', stopping: 'Остановка после текущего этапа…',
  reveal: 'Показать в Finder', result: 'MIDI сохранён · нот: {n}',
  stages: { preparing: 'Подготовка аудио…', downloading: 'Загрузка Basic Pitch…', transcribing: 'Распознавание нот…', notes: 'Формирование нот…', saving: 'Сохранение MIDI…', done: 'MIDI сохранён', cancelled: 'Остановлено', failed: 'Не удалось экспортировать MIDI' },
} satisfies typeof en
