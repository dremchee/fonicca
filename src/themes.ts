// Colour themes: 8 base colours per mode (light and dark); every colour variable
// in styles.css is derived from them. Built-in themes live here, custom ones in
// settings.json.

export type Mode = 'light' | 'dark'

export const COLOR_KEYS = ['background', 'surface', 'text', 'muted', 'border', 'accent', 'danger', 'wave'] as const
export type ColorKey = typeof COLOR_KEYS[number]
export type Palette = Record<ColorKey, string>

/** A custom theme as settings.json stores it. */
export interface CustomTheme {
  id: string
  name: string
  light: Palette
  dark: Palette
}

/** Built-in themes have no stored name: it comes from the dictionaries. */
export interface Theme extends CustomTheme {
  builtIn: boolean
}

export const DEFAULT_THEME = 'fonicca'

const p = (background: string, surface: string, text: string, muted: string, border: string, accent: string, danger: string, wave: string): Palette =>
  ({ background, surface, text, muted, border, accent, danger, wave })

export const BUILT_IN: Theme[] = [
  {
    id: 'fonicca',
    name: 'Fonicca',
    builtIn: true,
    light: p('#f6f6f8', '#ffffff', '#1c1c22', '#6e6e7a', '#e1e1e8', '#6d4aff', '#d92d4a', '#8b8b99'),
    dark: p('#141418', '#1b1b21', '#e8e8ee', '#8d8d9a', '#2c2c35', '#8b6dff', '#ff5c77', '#5c5c6a'),
  },
  {
    id: 'graphite',
    name: 'Graphite',
    builtIn: true,
    light: p('#f4f4f4', '#fbfbfb', '#1d1d1f', '#68686d', '#dddde0', '#4a6a8a', '#c8323c', '#8e8e93'),
    dark: p('#161617', '#1e1e20', '#e6e6e8', '#8e8e93', '#2e2e31', '#7d9bbd', '#f0646e', '#5a5a5f'),
  },
  {
    id: 'midnight',
    name: 'Midnight',
    builtIn: true,
    light: p('#eef3f8', '#fafcff', '#0f1d2e', '#53647a', '#d3dde9', '#0b7f96', '#d43a4f', '#7f93a8'),
    dark: p('#0b1220', '#111a2c', '#dbe7f5', '#7f93ab', '#1e2a40', '#2cc4d9', '#ff6b7d', '#3d5068'),
  },
  {
    id: 'tape',
    name: 'Tape',
    builtIn: true,
    light: p('#f5efe4', '#fbf7ef', '#2b2118', '#74644f', '#e3d8c5', '#a8650b', '#b83a2c', '#a8977f'),
    dark: p('#1a1612', '#221d17', '#efe5d4', '#a29179', '#352d23', '#e6a23c', '#f07560', '#6a5c4a'),
  },
  {
    id: 'moss',
    name: 'Moss',
    builtIn: true,
    light: p('#f1f4ef', '#fafcf8', '#1b2419', '#5b6a57', '#d9e1d4', '#3b8045', '#c23b3b', '#8a9a85'),
    dark: p('#10150f', '#171e16', '#dde8d9', '#8a9b85', '#263024', '#6cc47a', '#f06a6a', '#4b5a47'),
  },
  {
    id: 'neon',
    name: 'Neon',
    builtIn: true,
    light: p('#f7f3fb', '#ffffff', '#1e1630', '#6a5e80', '#e6dcf0', '#c42a7c', '#d43a3a', '#9a8fb0'),
    dark: p('#0d0a14', '#15111f', '#ece6f7', '#9488ad', '#261f36', '#ff4fa8', '#ff6b6b', '#3fd1c7'),
  },
]

// --- Colour maths ------------------------------------------------------------

function rgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '')
  const n = Number.parseInt(h.length === 3 ? h.split('').map(c => c + c).join('') : h, 16)
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255]
}

export function isHex(s: string) {
  return /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i.test(s.trim())
}

function luminance(hex: string) {
  const [r, g, b] = rgb(hex).map((v) => {
    const c = v / 255
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
  })
  return 0.2126 * r! + 0.7152 * g! + 0.0722 * b!
}

/** WCAG contrast ratio, 1..21. */
export function contrast(a: string, b: string) {
  const [l1, l2] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (l1! + 0.05) / (l2! + 0.05)
}

function mix(a: string, b: string, t: number) {
  const [x, y] = [rgb(a), rgb(b)]
  return `#${x.map((v, i) => Math.round(v + (y[i]! - v) * t).toString(16).padStart(2, '0')).join('')}`
}

function alpha(hex: string, a: number) {
  const [r, g, b] = rgb(hex)
  return `rgb(${r} ${g} ${b} / ${a})`
}

/** Text on the accent: whichever of white and near-black reads better. */
export function onAccent(accent: string) {
  return contrast(accent, '#ffffff') >= contrast(accent, '#111114') ? '#ffffff' : '#111114'
}

/** The full set of CSS variables styles.css defines, from the 8 base colours. */
export function cssVars(pal: Palette, mode: Mode): Record<string, string> {
  const subtle = mix(pal.background, pal.text, mode === 'light' ? 0.06 : 0.09)
  return {
    '--background': pal.background,
    '--foreground': pal.text,
    '--card': pal.surface,
    '--popover': pal.surface,
    '--primary': pal.accent,
    '--primary-foreground': onAccent(pal.accent),
    '--primary-soft': alpha(pal.accent, mode === 'light' ? 0.12 : 0.18),
    '--secondary': subtle,
    '--muted': subtle,
    '--accent': subtle,
    '--muted-foreground': pal.muted,
    '--destructive': pal.danger,
    '--border': pal.border,
    '--input': pal.border,
    '--wave': pal.wave,
  }
}

/** Which check failed: text needs 4.5:1, secondary text and controls 3:1. */
export type ContrastCheck = 'textOnBackground' | 'textOnSurface' | 'mutedOnBackground' | 'accentOnBackground' | 'textOnAccent' | 'dangerOnBackground'

export interface ContrastIssue {
  key: ColorKey
  check: ContrastCheck
  ratio: number
  min: number
}

export function contrastIssues(pal: Palette): ContrastIssue[] {
  const checks: [ColorKey, ContrastCheck, string, string, number][] = [
    ['text', 'textOnBackground', pal.text, pal.background, 4.5],
    ['text', 'textOnSurface', pal.text, pal.surface, 4.5],
    ['muted', 'mutedOnBackground', pal.muted, pal.background, 3],
    ['accent', 'accentOnBackground', pal.accent, pal.background, 3],
    ['accent', 'textOnAccent', onAccent(pal.accent), pal.accent, 4.5],
    ['danger', 'dangerOnBackground', pal.danger, pal.background, 3],
  ]
  return checks
    .map(([key, check, a, b, min]) => ({ key, check, ratio: contrast(a, b), min }))
    .filter(i => i.ratio < i.min)
}

/** The selected theme; an unknown id (e.g. a deleted custom theme) gives the default one. */
export function findTheme(id: string, custom: CustomTheme[]): Theme {
  const builtIn = BUILT_IN.find(t => t.id === id)
  if (builtIn)
    return builtIn
  const mine = custom.find(t => t.id === id)
  return mine ? { ...mine, builtIn: false } : BUILT_IN[0]!
}

/** Puts a palette on the page: CSS variables, the `dark` class and native controls. */
export function applyPalette(pal: Palette, mode: Mode, root = document.documentElement) {
  for (const [name, value] of Object.entries(cssVars(pal, mode)))
    root.style.setProperty(name, value)
  root.classList.toggle('dark', mode === 'dark')
  root.style.colorScheme = mode
}
