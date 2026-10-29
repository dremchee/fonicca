export const ROW_PLAYBACK_MODES = ['cursor', 'start', 'selected', 'alt'] as const
export type RowPlaybackMode = typeof ROW_PLAYBACK_MODES[number]

export function rowClickSeeks(mode: RowPlaybackMode, alreadySelected: boolean, altKey: boolean) {
  return mode === 'cursor'
    || (mode === 'selected' && alreadySelected)
    || (mode === 'alt' && altKey)
}
