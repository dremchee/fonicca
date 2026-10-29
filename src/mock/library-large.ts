import type { Sample, SampleFilter } from '../api'

/** On-demand metadata only: scrolling must not retain a synthetic 100k-row array. */
export function largeLibraryCommand(command: string, args: { filter?: SampleFilter, offset?: number, limit?: number }) {
  if (location.hash !== '#library-large') return undefined
  const size = new URLSearchParams(location.search).get('size') === '32000' ? 32000 : 100000
  if (command === 'list_sources') return [{ id: 1, path: '/fixture', sampleCount: size }]
  if (command === 'source_folders') return []
  if (command === 'sample_wave') return new ArrayBuffer(0)
  if (!['count_samples', 'list_samples', 'sample_ids'].includes(command)) return undefined
  const filter = args.filter
  const query = filter?.query?.trim().toLowerCase() ?? ''
  const matches = (id: number) => {
    const category = id % 2 ? 'kick' : 'snare'
    return (!filter?.sourceId || filter.sourceId === 1) && !filter?.favorites
      && (!filter?.category || filter.category === category)
      && (!query || `${category} ${id}`.includes(query))
  }
  // Scan integers, constructing objects only for the requested page.
  const ids: number[] = []
  for (let id = 1; id <= size; id++) if (matches(id)) ids.push(id)
  if (filter?.sortDirection === 'desc') ids.reverse()
  if (command === 'count_samples') return ids.length
  if (command === 'sample_ids') return ids.slice(args.offset ?? 0,
    args.limit == null ? undefined : (args.offset ?? 0) + args.limit)
  const rows: Sample[] = ids.slice(args.offset ?? 0, (args.offset ?? 0) + (args.limit ?? 200)).map(id => ({
    id, sourceId: 1, name: `${id % 2 ? 'Kick' : 'Snare'} ${id}`, path: `/fixture/Pack ${id % 100}/Sample ${id}.wav`,
    ext: 'wav', size: 40000, duration: 0.5, sampleRate: 44100, channels: 2, bitDepth: 24,
    favorite: false, tags: [], kind: 'oneshot', bpm: 120, bpmSource: 'audio', key: null, keySource: null,
    peakDb: -1, lufs: -18, analyzed: true, category: id % 2 ? 'kick' : 'snare', categorySource: 'audio',
    categoryConfidence: 1, similarity: null, stemCount: 0,
  }))
  return new Promise(resolve => setTimeout(() => resolve(rows), 25))
}
