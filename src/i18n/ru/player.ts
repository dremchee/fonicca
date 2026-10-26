import type en from '../en/player'

export default {
  volume: 'Громкость',
  hiddenTags: 'Скрытые теги: {n}',
  seek: 'Позиция воспроизведения',
  seekHint: 'Стрелки — перемотка, Shift — больший шаг, Home и End — начало и конец. Пробел — играть или приостановить.',
  position: '{elapsed} из {duration}',
  space: 'Пробел',
  dragToDaw: 'Перетащите в DAW',
  edit: 'Редактировать',
  removeTag: 'Убрать тег {name}',
  autoplayShort: 'Авто',
  autoplay: 'Автовоспроизведение',
  readingWaveform: 'Чтение формы волны…',
  samplePeak: 'Пиковый уровень',
  integratedLoudness: 'Интегральная громкость',
  clickToReveal: 'Нажмите, чтобы показать в Finder',
  wavePreview: 'Предпросмотр формы волны',
  cover: 'Обложка',
  meter: {
    noClip: 'Перегрузки нет',
    title: 'Уровень на выходе, дБFS',
    clip: 'Перегрузка. Нажмите, чтобы сбросить',
  },
} satisfies typeof en
