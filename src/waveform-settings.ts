import type { WaveformSettings } from './api'

export const WAVEFORM_DEFAULTS: WaveformSettings = {
  waveStyle: 'peaksRms', waveChannels: 'split', waveScale: 'linear', waveColors: 'theme',
  waveDensity: 'off', wavePeakOpacity: 0.4, waveRmsOpacity: 1, waveThickness: 2,
  waveBarWidth: 2, waveBarGap: 1, waveBarRadius: 0,
  waveDbRange: 60, waveChannelColors: [null, null],
}

export const WAVEFORM_PRESETS = {
  editing: { ...WAVEFORM_DEFAULTS },
  detail: { ...WAVEFORM_DEFAULTS, waveScale: 'db', waveDbRange: 90, waveDensity: 'laplace' },
  music: { ...WAVEFORM_DEFAULTS, waveColors: 'frequency', waveDensity: 'gaussian' },
} satisfies Record<string, WaveformSettings>

export type WaveformPreset = keyof typeof WAVEFORM_PRESETS

export function waveformSettings(value: Partial<WaveformSettings> = {}): WaveformSettings {
  const settings = { ...WAVEFORM_DEFAULTS }
  for (const key of Object.keys(WAVEFORM_DEFAULTS) as (keyof WaveformSettings)[]) {
    if (value[key] !== undefined)
      Object.assign(settings, { [key]: value[key] })
  }
  settings.waveChannelColors = [...(value.waveChannelColors ?? WAVEFORM_DEFAULTS.waveChannelColors)]
  return settings
}

export function waveformPreset(value: WaveformSettings): WaveformPreset | 'custom' {
  return (Object.keys(WAVEFORM_PRESETS) as WaveformPreset[]).find((key) => {
    const preset = WAVEFORM_PRESETS[key]
    return (Object.keys(preset) as (keyof WaveformSettings)[]).every(field => JSON.stringify(value[field]) === JSON.stringify(preset[field]))
  }) ?? 'custom'
}

/** Shared appearance mapping for library, editor, minimap and settings examples. */
export function waveformLook(value: WaveformSettings) {
  return {
    style: value.waveStyle, scale: value.waveScale, colors: value.waveColors,
    density: value.waveDensity, peakOpacity: value.wavePeakOpacity, rmsOpacity: value.waveRmsOpacity,
    barWidth: value.waveBarWidth, barGap: value.waveBarGap, barRadius: value.waveBarRadius,
    thickness: value.waveThickness, dbRange: value.waveDbRange, channelColors: value.waveChannelColors,
  }
}
