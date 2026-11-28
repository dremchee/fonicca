import { defineConfig } from 'vitepress'

const repo = 'https://github.com/dremchee/fonicca'
const downloads = `${repo}/releases`

export default defineConfig({
  title: 'Fonicca',
  description: 'Documentation for the Fonicca desktop audio library and editor',
  srcExclude: ['architecture.md', 'builds.md', 'dependencies.md', 'releases/**'],
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
          { text: 'Guide', link: '/user-guide', activeMatch: '^/(installation|user-guide)' },
          { text: 'Release notes', link: '/changelog' },
          { text: 'Download', link: downloads },
        ],
        sidebar: [
          {
            text: 'Using Fonicca',
            items: [
              { text: 'Installation', link: '/installation' },
              { text: 'User guide', link: '/user-guide' },
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
          { text: 'Руководство', link: '/ru/user-guide', activeMatch: '^/ru/(installation|user-guide)' },
          { text: 'Что нового', link: '/ru/changelog' },
          { text: 'Скачать', link: downloads },
        ],
        sidebar: [
          {
            text: 'Работа с Fonicca',
            items: [
              { text: 'Установка', link: '/ru/installation' },
              { text: 'Руководство', link: '/ru/user-guide' },
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
