/**
 * Colour math for the colour picker, backed by culori.
 *
 * The picker keeps its source of truth in HSVA because the 2D selection area
 * lives in HSV space — round-tripping through other models on every drag would
 * make the handle jump whenever value or saturation hits an extreme (hue
 * becomes ambiguous). culori handles parsing, conversion and gamut mapping for
 * the wider spaces (OKLCH, Display-P3); everything below is a thin, typed shim
 * that converts to/from our 0..360 / 0..100 / 0..1 HSVA convention.
 *
 * We import from `culori/fn` and register only the modes we use so the bundle
 * stays small (the default `culori` entry pulls in every colour space).
 */
import {
  converter,
  formatHex,
  formatHex8,
  formatHsl,
  formatRgb,
  modeHsl,
  modeHsv,
  modeOklch,
  modeP3,
  modeRgb,
  parse,
  toGamut,
  useMode,
} from 'culori/fn'

useMode(modeRgb)
useMode(modeHsv)
useMode(modeHsl)
useMode(modeOklch)
useMode(modeP3)

const toRgb = converter('rgb')
const toHsv = converter('hsv')
const toHsl = converter('hsl')
const toOklch = converter('oklch')
const toP3 = converter('p3')

/** Hue 0..360, saturation/value 0..100, alpha 0..1. */
export interface HSVA {
  h: number
  s: number
  v: number
  a: number
}

/** All formats the picker can output / edit, in switcher order. */
export const COLOR_FORMATS = ['hex', 'rgb', 'hsl', 'oklch', 'p3'] as const
export type ColorFormat = typeof COLOR_FORMATS[number]

export function clamp(n: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, n))
}

function round(n: number, precision = 0): number {
  const f = 10 ** precision
  return Math.round(n * f) / f
}

function toCuloriHsv(c: HSVA) {
  return { mode: 'hsv' as const, h: c.h, s: c.s / 100, v: c.v / 100, alpha: c.a }
}

/**
 * Convert any culori colour back to HSVA, falling back to the previous hue when
 * the result is achromatic (culori reports `h` as `undefined` for greys, which
 * would otherwise snap the hue slider to 0).
 */
function fromCulori(color: Parameters<typeof toHsv>[0], prev: HSVA): HSVA {
  const hsv = toHsv(color)
  if (!hsv)
    return prev
  return {
    h: hsv.h ?? prev.h,
    s: (hsv.s ?? 0) * 100,
    v: (hsv.v ?? 0) * 100,
    a: hsv.alpha ?? prev.a,
  }
}

/**
 * Infer the notation of a CSS colour string. Leans on culori's parser (the
 * `mode` of the parse result), except inside the rgb family, where hex /
 * `rgb()` / named colours all parse to mode `rgb` and only the source syntax
 * tells them apart. Returns null when the notation isn't pinned down (named
 * colours, unparseable input, modes the picker doesn't offer) so the caller
 * can fall back to its default format.
 */
export function detectColorFormat(input: string): ColorFormat | null {
  const str = input.trim()
  const parsed = str ? parse(str) : undefined
  if (!parsed)
    return null
  switch (parsed.mode) {
    case 'hsl':
      return 'hsl'
    case 'oklch':
      return 'oklch'
    case 'p3':
      return 'p3'
    case 'rgb':
      if (str.startsWith('#'))
        return 'hex'
      if (/^rgba?\(/i.test(str))
        return 'rgb'
      return null
    default:
      return null
  }
}

/** Parse any CSS colour string (hex, rgb, hsl, oklch, display-p3, …). */
export function parseColor(input: string): HSVA | null {
  const str = input.trim()
  if (!str)
    return null
  const parsed = parse(str)
  if (!parsed)
    return null
  return fromCulori(parsed, { h: 0, s: 0, v: 0, a: 1 })
}

// Deliberately NOT culori's formatCss: that emits unrounded floats and a
// 0..1 lightness, while the picker wants short values and a percent L —
// these two formatters are presentation, not colour math.
function formatOklch(c: HSVA): string {
  const o = toOklch(toCuloriHsv(c))
  const l = round((o.l ?? 0) * 100, 2)
  const ch = round(o.c ?? 0, 4)
  const h = round(o.h ?? 0, 2)
  return c.a < 1
    ? `oklch(${l}% ${ch} ${h} / ${round(c.a, 2)})`
    : `oklch(${l}% ${ch} ${h})`
}

function formatP3(c: HSVA): string {
  const p = toP3(toCuloriHsv(c))
  const r = round(p.r ?? 0, 4)
  const g = round(p.g ?? 0, 4)
  const b = round(p.b ?? 0, 4)
  return c.a < 1
    ? `color(display-p3 ${r} ${g} ${b} / ${round(c.a, 2)})`
    : `color(display-p3 ${r} ${g} ${b})`
}

/** Serialise an HSVA colour to a CSS string in the requested format. */
export function formatColor(c: HSVA, format: ColorFormat): string {
  const hsv = toCuloriHsv(c)
  switch (format) {
    case 'rgb':
      return formatRgb(hsv)
    case 'hsl':
      return formatHsl(hsv)
    case 'oklch':
      return formatOklch(c)
    case 'p3':
      return formatP3(c)
    case 'hex':
    default:
      return toHex(c)
  }
}

/**
 * CSS colour string for swatches and gradients. culori's serializer already
 * clamps channels into sRGB and rounds, so this is a thin alias.
 */
export function toCssColor(c: HSVA): string {
  return formatRgb(toCuloriHsv(c)) ?? 'rgb(0, 0, 0)'
}

// --- numeric channel editing ----------------------------------------------

export interface ChannelDef {
  key: string
  label: string
  value: number
  min: number
  max: number
  step: number
}

/** Channel descriptors for the numeric inputs of a given format (excludes alpha). */
export function getChannels(c: HSVA, format: ColorFormat): ChannelDef[] {
  if (format === 'rgb') {
    const rgb = toRgb(toCuloriHsv(c))
    const ch = (n: number | undefined) => clamp(Math.round((n ?? 0) * 255), 0, 255)
    return [
      { key: 'r', label: 'R', value: ch(rgb.r), min: 0, max: 255, step: 1 },
      { key: 'g', label: 'G', value: ch(rgb.g), min: 0, max: 255, step: 1 },
      { key: 'b', label: 'B', value: ch(rgb.b), min: 0, max: 255, step: 1 },
    ]
  }
  if (format === 'hsl') {
    const hsl = toHsl(toCuloriHsv(c))
    return [
      { key: 'h', label: 'H', value: round(hsl.h ?? 0), min: 0, max: 360, step: 1 },
      { key: 's', label: 'S', value: round((hsl.s ?? 0) * 100), min: 0, max: 100, step: 1 },
      { key: 'l', label: 'L', value: round((hsl.l ?? 0) * 100), min: 0, max: 100, step: 1 },
    ]
  }
  if (format === 'oklch') {
    const o = toOklch(toCuloriHsv(c))
    return [
      { key: 'l', label: 'L', value: round((o.l ?? 0) * 100, 1), min: 0, max: 100, step: 0.1 },
      { key: 'c', label: 'C', value: round(o.c ?? 0, 3), min: 0, max: 0.4, step: 0.001 },
      { key: 'h', label: 'H', value: round(o.h ?? 0), min: 0, max: 360, step: 1 },
    ]
  }
  if (format === 'p3') {
    const p = toP3(toCuloriHsv(c))
    const ch = (n: number | undefined) => clamp(Math.round((n ?? 0) * 255), 0, 255)
    return [
      { key: 'r', label: 'R', value: ch(p.r), min: 0, max: 255, step: 1 },
      { key: 'g', label: 'G', value: ch(p.g), min: 0, max: 255, step: 1 },
      { key: 'b', label: 'B', value: ch(p.b), min: 0, max: 255, step: 1 },
    ]
  }
  return []
}

/**
 * Apply a single edited channel and return the new HSVA. Wide-gamut edits
 * (OKLCH/P3) are mapped back into sRGB — the picker's spatial model is HSV, so
 * out-of-sRGB values can't be represented and are gamut-mapped for validity.
 */
export function setChannel(c: HSVA, format: ColorFormat, key: string, value: number): HSVA {
  if (format === 'rgb') {
    const rgb = toRgb(toCuloriHsv(c))
    rgb[key as 'r' | 'g' | 'b'] = clamp(value, 0, 255) / 255
    rgb.alpha = c.a
    return fromCulori(rgb, c)
  }
  if (format === 'hsl') {
    const hsl = toHsl(toCuloriHsv(c))
    if (key === 'h')
      hsl.h = value
    else
      hsl[key as 's' | 'l'] = clamp(value, 0, 100) / 100
    hsl.alpha = c.a
    return fromCulori(hsl, c)
  }
  if (format === 'oklch') {
    const o = toOklch(toCuloriHsv(c))
    if (key === 'l')
      o.l = clamp(value, 0, 100) / 100
    else if (key === 'c')
      o.c = clamp(value, 0, 0.5)
    else
      o.h = value
    o.alpha = c.a
    return fromCulori(toGamut('rgb', 'oklch')(o), c)
  }
  if (format === 'p3') {
    const p = toP3(toCuloriHsv(c))
    p[key as 'r' | 'g' | 'b'] = clamp(value, 0, 255) / 255
    p.alpha = c.a
    return fromCulori(toGamut('rgb', 'p3')(p), c)
  }
  return c
}

/** Parse a hex string (with or without leading `#`); `null` if invalid. */
export function parseHex(input: string): HSVA | null {
  const str = input.trim()
  return parseColor(str.startsWith('#') ? str : `#${str}`)
}

/** Current colour as a hex string (`#rrggbb` or `#rrggbbaa`). */
export function toHex(c: HSVA, withAlpha = true): string {
  const hsv = toCuloriHsv(c)
  return withAlpha && c.a < 1 ? formatHex8(hsv) : formatHex(hsv)
}

/** Inline-style object that renders the classic transparency checkerboard. */
export const CHECKERBOARD_STYLE = {
  backgroundImage:
    'linear-gradient(45deg, #c7ccd4 25%, transparent 25%), linear-gradient(-45deg, #c7ccd4 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #c7ccd4 75%), linear-gradient(-45deg, transparent 75%, #c7ccd4 75%)',
  backgroundSize: '8px 8px',
  backgroundPosition: '0 0, 0 4px, 4px -4px, -4px 0',
  backgroundColor: '#ffffff',
} as const
