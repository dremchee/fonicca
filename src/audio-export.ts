export const AUDIO_EXPORT_FORMATS = ['wav', 'mp3', 'ogg', 'flac', 'aiff', 'm4a', 'caf'] as const
export type AudioExportFormat = typeof AUDIO_EXPORT_FORMATS[number]
export const AUDIO_EXPORT_LABELS: Record<AudioExportFormat, string> = {
  wav: 'WAV', mp3: 'MP3', ogg: 'OGG Vorbis', flac: 'FLAC', aiff: 'AIFF', m4a: 'M4A (ALAC)', caf: 'CAF',
}

export function isAudioExportFormat(value: unknown): value is AudioExportFormat {
  return AUDIO_EXPORT_FORMATS.includes(value as AudioExportFormat)
}

export interface AudioExportOptions { format: AudioExportFormat, sampleRate: number | null }
export const AUDIO_EXPORT_SAMPLE_RATES = [22050, 44100, 48000, 88200, 96000] as const
export const MP3_SAMPLE_RATES = [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000]

export function supportedExportRate(format: AudioExportFormat, rate: number | null, sourceRate: number): number | null {
  return format === 'mp3' && !MP3_SAMPLE_RATES.includes(rate ?? sourceRate) ? 48000 : rate
}

const extensionPattern = /\.([^./\\]+)$/
const formatAliases = new Map<string, AudioExportFormat>([
  ['wave', 'wav'],
  ['aif', 'aiff'],
  ['aifc', 'aiff'],
  ['oga', 'ogg'],
  ['mp4', 'm4a'],
])

function formatFromExtension(extension = '') {
  const ext = extension.toLowerCase()
  return isAudioExportFormat(ext) ? ext : formatAliases.get(ext)
}

export function audioExportFormat(path: string): AudioExportFormat {
  return formatFromExtension(extensionPattern.exec(path)?.[1]) ?? 'wav'
}

/** The selected encoder and the final file extension must always agree. */
export function audioExportPath(path: string, format: AudioExportFormat): string {
  const suffix = extensionPattern.exec(path)
  const base = suffix && formatFromExtension(suffix[1]) ? path.slice(0, suffix.index) : path
  return `${base}.${format}`
}
