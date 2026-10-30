import { modelCatalog } from '../../model-catalog.generated'
import type en from '../en/stems'
export default {
  badge: "{n}",
  expand: "Показать стемы: {name}",
  collapse: "Скрыть стемы: {name}",
  folderSettings: "Настройки…",
  storageTitle: "Папка для стемов",
  storageDescription: "Для каждого трека создаётся отдельная папка с его названием. Новые стемы раскрываются под исходным треком в библиотеке независимо от папки сохранения. Уже созданные файлы остаются на своих местах.",
  besideOriginal: "Рядом с исходным треком",
  useOriginalFolder: "Рядом с исходником",

  title: 'Стемы', action: 'Разделить на стемы…', description: 'Локальное разделение трека. Исходник сохраняется.',
  mode: 'Дорожки', twoTracks: '2 дорожки', fourTracks: '4 дорожки',
  twoDescription: 'Вокал и минус', fourDescription: 'Вокал, барабаны, бас и остальные инструменты',
  model: 'Модель разделения',
  models: { melRoformer: `${modelCatalog.melRoformer.name}`, bsRoformer: `${modelCatalog.bsRoformer.name}`, mdx23c: `${modelCatalog.mdx23c.name}`, htdemucs: `${modelCatalog.htdemucs.name} · 4 стема`, scnetXlIhf: `${modelCatalog.scnetXlIhf.name} · Качество`, scnetLarge: `${modelCatalog.scnetLarge.name} · Баланс`, spleeter: `${modelCatalog.spleeter.name} · Быстро` },
  modelDescriptions: { melRoformer: 'Специализированная модель для выделения вокала и сохранения минуса.', bsRoformer: 'Специализированная модель для вокала и минуса. Полная точность весов.', mdx23c: 'Разделение на вокал и минус. Обработка на этом компьютере.', htdemucs: 'Вокал, барабаны, бас и остальные инструменты. Предобученные веса — только для научных целей.', scnetXlIhf: 'Детальное разделение. Выше нагрузка на компьютер.', scnetLarge: 'Для более детального разделения. Требует больше памяти и времени, особенно на длинных треках.', spleeter: 'Быстрое разделение с меньшей нагрузкой на компьютер.' },
  folder: 'Папка результата', choose: 'Выбрать…', noFolder: 'Выберите папку для WAV-файлов результата.',
  download: 'При первом запуске загрузится выбранная модель ({size}). Аудио остаётся на этом компьютере.',
  ready: 'Модель установлена · локальная обработка', limits: 'Моно или стерео, до 10 минут. Результат — стерео WAV 44,1 кГц, 32-bit float.',
  start: 'Разделить', downloadStart: 'Скачать модель и разделить', stop: 'Остановить', stopping: 'Остановка после текущего этапа…',
  close: 'Закрыть', reveal: 'Показать в Finder', edit: 'Открыть в редакторе',
  stages: { preparing: 'Подготовка звука…', downloading: 'Загрузка модели…', separating: 'Разделение на стемы…', importing: 'Добавление в библиотеку…', done: 'Стемы добавлены в библиотеку', cancelled: 'Остановлено', failed: 'Не удалось разделить трек' },
  instruments: { vocals: 'Вокал', drums: 'Барабаны', bass: 'Бас', other: 'Остальное', instrumental: 'Минус' },
} satisfies typeof en
