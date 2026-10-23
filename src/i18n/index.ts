import { createI18n } from 'vue-i18n'
import en from './en'
import ru from './ru'

export type Locale = 'en' | 'ru'
export const LOCALES: { id: Locale, name: string }[] = [
  { id: 'en', name: 'English' },
  { id: 'ru', name: 'Русский' },
]

/**
 * Russian plurals have three forms: "1 семпл", "2 семпла", "5 семплов".
 * Messages list them in that order: "{n} семпл | {n} семпла | {n} семплов".
 */
function russianPlural(choice: number, choicesLength: number) {
  if (choicesLength < 3)
    return choice === 1 ? 0 : 1
  const n = Math.abs(choice)
  if (n % 10 === 1 && n % 100 !== 11)
    return 0
  if (n % 10 >= 2 && n % 10 <= 4 && (n % 100 < 12 || n % 100 > 14))
    return 1
  return 2
}

export const i18n = createI18n<[typeof en], Locale, false>({
  legacy: false,
  locale: 'en',
  fallbackLocale: 'en',
  messages: { en, ru },
  pluralRules: { ru: russianPlural },
})

/** For stores and helpers outside components; components use `useI18n()`. */
export const t = i18n.global.t

export function setLocale(locale: Locale) {
  i18n.global.locale.value = locale
  document.documentElement.lang = locale
}
