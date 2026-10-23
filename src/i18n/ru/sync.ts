import type en from '../en/sync'

export default {
  button: 'Синхр.',
  hint: 'Прослушивание в темпе и тональности проекта',
  hintApplied: 'Играет с подстройкой: {applied}',
  ariaLabel: 'Темп и тональность проекта',
  match: 'Подстраивать под проект',
  description: 'Лупы растягиваются под темп проекта, семплы с известной тональностью транспонируются в тональность проекта.',
  tempo: 'Темп',
  key: 'Тональность',
  projectKey: 'Тональность проекта',
  none: 'Нет',
  major: 'Мажор',
  minor: 'Минор',
  appliedTempo: 'темп ×{n}',
  appliedSemitones: '{n} полут.',
} satisfies typeof en
