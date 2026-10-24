import type en from '../en/menu'

// Wording of the standard items as macOS shows them in Russian.
export default {
  app: {
    about: 'О программе Fonicca',
    settings: 'Настройки…',
    services: 'Службы',
    hide: 'Скрыть Fonicca',
    hideOthers: 'Скрыть остальные',
    showAll: 'Показать все',
    quit: 'Завершить Fonicca',
  },
  file: {
    title: 'Файл',
    closeWindow: 'Закрыть окно',
  },
  edit: {
    title: 'Правка',
    undo: 'Отменить',
    redo: 'Повторить',
    cut: 'Вырезать',
    copy: 'Скопировать',
    paste: 'Вставить',
    selectAll: 'Выбрать все',
  },
  view: {
    title: 'Вид',
    fullscreen: 'Перейти в полноэкранный режим',
  },
  window: {
    title: 'Окно',
    minimize: 'Свернуть',
    zoom: 'Изменить масштаб',
  },
  help: {
    title: 'Справка',
  },
} satisfies typeof en
