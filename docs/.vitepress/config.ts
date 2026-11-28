import { defineConfig } from 'vitepress'

const repo = 'https://github.com/dremchee/fonicca'
const downloads = `${repo}/releases`

export default defineConfig({
  title: 'Fonicca',
  description: 'Documentation for the Fonicca desktop audio library and editor',
  srcExclude: ['architecture.md', 'builds.md', 'dependencies.md', 'releases/**'],
  transformPageData(pageData, { siteConfig }) {
    pageData.frontmatter.head ??= []
    pageData.frontmatter.head.push(['link', { rel: 'icon', type: 'image/png', href: `${siteConfig.site.base}favicon.png` }])
  },
  themeConfig: {
    socialLinks: [{ icon: 'github', link: repo }],
    outline: { level: [2, 3] },
    search: {
      provider: 'local',
      options: {
        locales: {
          ru: {
            translations: {
              button: { buttonText: 'Поиск', buttonAriaLabel: 'Поиск по документации' },
              modal: {
                displayDetails: 'Показать подробности',
                resetButtonTitle: 'Очистить запрос',
                backButtonTitle: 'Закрыть поиск',
                noResultsText: 'Ничего не найдено',
                footer: {
                  selectText: 'открыть',
                  selectKeyAriaLabel: 'Ввод',
                  navigateText: 'выбрать',
                  navigateUpKeyAriaLabel: 'Стрелка вверх',
                  navigateDownKeyAriaLabel: 'Стрелка вниз',
                  closeText: 'закрыть',
                  closeKeyAriaLabel: 'Esc',
                },
              },
            },
          },
        },
      },
    },
  },
  locales: {
    root: {
      label: 'English',
      lang: 'en',
      themeConfig: {
        nav: [
          { text: 'Guide', link: '/user-guide', activeMatch: '^/(installation|user-guide|guide/)' },
          { text: 'Release notes', link: '/changelog' },
          { text: 'Download', link: downloads },
        ],
        sidebar: [
          {
            text: 'Using Fonicca',
            items: [
              { text: 'Installation', link: '/installation' },
              { text: 'User guide', link: '/user-guide' },
              { text: 'Settings', link: '/guide/settings' },
            ],
          },
          {
            text: 'Library',
            items: [
              { text: 'Folders and table', link: '/guide/library' },
              { text: 'Search and filters', link: '/guide/search' },
              { text: 'Favorites, collections and tags', link: '/guide/organize' },
              { text: 'Audition and drag to DAW', link: '/guide/playback' },
              { text: 'Track information', link: '/guide/track-info' },
              { text: 'Duplicates', link: '/guide/duplicates' },
              { text: 'Transfer library data', link: '/guide/library-data' },
            ],
          },
          {
            text: 'Audio editor',
            items: [
              { text: 'Edit and process audio', link: '/guide/editor' },
              { text: 'Save and export audio', link: '/guide/export' },
              { text: 'Loops and slicing', link: '/guide/loops-slices' },
            ],
          },
          {
            text: 'Processing',
            items: [
              { text: 'Stem separation', link: '/guide/stems' },
              { text: 'Batch processing', link: '/guide/batch' },
              { text: 'Apollo restoration', link: '/guide/restoration' },
              { text: 'Audio to MIDI', link: '/guide/audio-to-midi' },
            ],
          },
          {
            text: 'Project',
            items: [
              { text: 'Release notes', link: '/changelog' },
              { text: 'Development', link: '/development' },
            ],
          },
        ],
      },
    },
    ru: {
      label: 'Русский',
      lang: 'ru',
      description: 'Документация Fonicca для работы с аудиобиблиотекой и редактором',
      themeConfig: {
        nav: [
          { text: 'Руководство', link: '/ru/user-guide', activeMatch: '^/ru/(installation|user-guide|guide/)' },
          { text: 'Что нового', link: '/ru/changelog' },
          { text: 'Скачать', link: downloads },
        ],
        sidebar: [
          {
            text: 'Работа с Fonicca',
            items: [
              { text: 'Установка', link: '/ru/installation' },
              { text: 'Руководство', link: '/ru/user-guide' },
              { text: 'Настройки', link: '/ru/guide/settings' },
            ],
          },
          {
            text: 'Библиотека',
            items: [
              { text: 'Папки и таблица', link: '/ru/guide/library' },
              { text: 'Поиск и фильтры', link: '/ru/guide/search' },
              { text: 'Избранное, коллекции и теги', link: '/ru/guide/organize' },
              { text: 'Прослушивание и передача в DAW', link: '/ru/guide/playback' },
              { text: 'Информация о треке', link: '/ru/guide/track-info' },
              { text: 'Дубликаты', link: '/ru/guide/duplicates' },
              { text: 'Перенос данных библиотеки', link: '/ru/guide/library-data' },
            ],
          },
          {
            text: 'Аудиоредактор',
            items: [
              { text: 'Редактирование и обработка', link: '/ru/guide/editor' },
              { text: 'Сохранение и экспорт', link: '/ru/guide/export' },
              { text: 'Петли и нарезка', link: '/ru/guide/loops-slices' },
            ],
          },
          {
            text: 'Обработка',
            items: [
              { text: 'Разделение на стемы', link: '/ru/guide/stems' },
              { text: 'Пакетная обработка', link: '/ru/guide/batch' },
              { text: 'Восстановление через Apollo', link: '/ru/guide/restoration' },
              { text: 'Аудио в MIDI', link: '/ru/guide/audio-to-midi' },
            ],
          },
          {
            text: 'Проект',
            items: [
              { text: 'Что нового', link: '/ru/changelog' },
              { text: 'Разработка', link: '/ru/development' },
            ],
          },
        ],
        outline: { level: [2, 3], label: 'На этой странице' },
        docFooter: { prev: 'Предыдущая страница', next: 'Следующая страница' },
        sidebarMenuLabel: 'Разделы',
        returnToTopLabel: 'Наверх',
        langMenuLabel: 'Язык',
        darkModeSwitchLabel: 'Оформление',
        lightModeSwitchTitle: 'Включить светлую тему',
        darkModeSwitchTitle: 'Включить тёмную тему',
        skipToContentLabel: 'Перейти к содержимому',
      },
    },
  },
})
