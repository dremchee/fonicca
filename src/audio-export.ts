export const AUDIO_EXPORT_FORMATS = ['wav', 'mp3', 'ogg'] as const
export type AudioExportFormat = typeof AUDIO_EXPORT_FORMATS[number]
export interface AudioExportOptions { format: AudioExportFormat, sampleRate: number | null }
export const AUDIO_EXPORT_SAMPLE_RATES = [22050, 44100, 48000, 88200, 96000] as const
export const MP3_SAMPLE_RATES = [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000]

export function supportedExportRate(format: AudioExportFormat, rate: number | null, sourceRate: number): number | null {
  return format === 'mp3' && !MP3_SAMPLE_RATES.includes(rate ?? sourceRate) ? 48000 : rate
}

export function audioExportFormat(path: string): AudioExportFormat {
  const ext = path.split('.').pop()?.toLowerCase()
  return AUDIO_EXPORT_FORMATS.find(format => format === ext) ?? 'wav'
}

/** The selected encoder and the final file extension must always agree. */
export function audioExportPath(path: string, format: AudioExportFormat): string {
  if (/\.(wav|mp3|ogg)$/i.test(path)) return path.replace(/\.(wav|mp3|ogg)$/i, `.${format}`)
  return `${path}.${format}`
}
