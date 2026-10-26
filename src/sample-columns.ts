export const SAMPLE_COLUMNS = [
  { id: 'directory', width: 180, defaultVisible: false },
  { id: 'tags', width: 120, defaultVisible: false },
  { id: 'category', width: 96, defaultVisible: true },
  { id: 'kind', width: 64, defaultVisible: false },
  { id: 'bpm', width: 56, defaultVisible: true },
  { id: 'key', width: 52, defaultVisible: false },
  { id: 'length', width: 64, defaultVisible: true },
  { id: 'type', width: 44, defaultVisible: false },
] as const

export type SampleColumnId = typeof SAMPLE_COLUMNS[number]['id']
export type SampleColumnVisibility = Record<SampleColumnId, boolean>

export function defaultSampleColumns(all = false): SampleColumnVisibility {
  return Object.fromEntries(SAMPLE_COLUMNS.map(c => [c.id, all || c.defaultVisible])) as SampleColumnVisibility
}

export type ResizableSampleColumnId = 'name' | SampleColumnId
export type SampleColumnWidths = Partial<Record<ResizableSampleColumnId, number>>

export function sampleColumnLimits(id: ResizableSampleColumnId) {
  return id === 'name' ? { min: 160, max: 2400 } : { min: id === 'directory' || id === 'tags' || id === 'category' ? 64 : 44, max: 800 }
}

export function boundedSampleColumnWidth(id: ResizableSampleColumnId, width: number): number {
  const limits = sampleColumnLimits(id)
  const fallback = id === 'name' ? 220 : SAMPLE_COLUMNS.find(c => c.id === id)!.width
  return Math.round(Math.min(limits.max, Math.max(limits.min, Number.isFinite(width) ? width : fallback)))
}

export function sampleTableLayout(columns: readonly typeof SAMPLE_COLUMNS[number][], widths: SampleColumnWidths, viewportWidth?: number) {
  const fixedName = widths.name != null && Number.isFinite(widths.name)
  let nameWidth = fixedName ? boundedSampleColumnWidth('name', widths.name!) : 220
  const sizes = columns.map(c => boundedSampleColumnWidth(c.id, widths[c.id] ?? c.width))
  // Playback keeps one compact slot; disclosure is absolute in the left padding.
  const overhead = 16 + 28 + 24 + (columns.length + 2) * 12
  if (viewportWidth != null && Number.isFinite(viewportWidth) && viewportWidth > 0) {
    let overflow = Math.max(0, overhead + nameWidth + sizes.reduce((sum, size) => sum + size, 0) - Math.floor(viewportWidth))
    // Prefer keeping metadata readable by using excess name space first.
    const fromName = Math.min(overflow, nameWidth - sampleColumnLimits('name').min)
    nameWidth -= fromName
    overflow -= fromName
    const capacity = sizes.map((size, i) => size - sampleColumnLimits(columns[i].id).min)
    let remaining = capacity.reduce((sum, size) => sum + size, 0)
    for (let i = 0; i < sizes.length && overflow > 0 && remaining > 0; i++) {
      const reduction = Math.min(capacity[i], Math.ceil(overflow * capacity[i] / remaining))
      sizes[i] -= reduction
      overflow -= reduction
      remaining -= capacity[i]
    }
  }
  return {
    '--sample-columns': `16px ${fixedName ? `${nameWidth}px` : `minmax(${nameWidth}px, 1fr)`} ${sizes.map(w => `${w}px`).join(' ')} ${fixedName ? 'minmax(28px, 1fr)' : '28px'}`,
    minWidth: `${overhead + nameWidth + sizes.reduce((sum, width) => sum + width, 0)}px`,
  }
}

export interface SampleSort {
  by: Exclude<ResizableSampleColumnId, 'directory'>
  direction: 'asc' | 'desc'
}

export function normalizeSampleSort(value: unknown): SampleSort {
  const sort = value && typeof value === 'object' ? value as Partial<SampleSort> : {}
  return {
    by: sort.by === 'name' || SAMPLE_COLUMNS.some(column => column.id !== 'directory' && column.id === sort.by) ? sort.by! : 'name',
    direction: sort.direction === 'desc' ? 'desc' : 'asc',
  }
}

export function sampleDirectory(path: string): string {
  const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return index < 0 ? '' : path.slice(0, index) || path.slice(0, 1)
}

export function relativeSampleDirectory(path: string, sourceRoot?: string): string {
  const directory = sampleDirectory(path)
  const root = sourceRoot?.replace(/[\\/]+$/, '')
  if (root && directory === root)
    return root.split(/[\\/]/).pop() ?? root
  if (root && (directory.startsWith(`${root}/`) || directory.startsWith(`${root}\\`)))
    return directory.slice(root.length + 1)
  return directory.split(/[\\/]/).filter(Boolean).pop() ?? directory
}
