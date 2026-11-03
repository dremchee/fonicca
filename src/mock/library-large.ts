import type { Sample, SampleFilter, TagRef } from '../api'

const tags: TagRef[] = []
const sampleTags = new Map<number, TagRef[]>()

/** On-demand metadata only: scrolling must not retain a synthetic 100k-row array. */
export function largeLibraryCommand(command: string, args: { filter?: SampleFilter, offset?: number, limit?: number, ids?: number[], id?: number, name?: string, tagId?: number }) {
  if (location.hash !== '#library-large') return undefined
  const size = new URLSearchParams(location.search).get('size') === '32000' ? 32000 : 100000
  const withStems = new URLSearchParams(location.search).get('stems') === '1'
  if (command === 'list_sources') return [{ id: 1, path: '/fixture', sampleCount: size }]
  if (command === 'source_folders') return []
  if (command === 'sample_wave') {
    if (!withStems) return new ArrayBuffer(0)
    const count = 64, bytes = new ArrayBuffer(16 + count * 12)
    new Uint32Array(bytes, 0, 4).set([0, 1, count, 6])
    const points = new Int16Array(bytes, 16)
    for (let i = 0; i < count; i++) {
      const peak = Math.round((0.15 + 0.7 * Math.abs(Math.sin((i + (args.id ?? 0)) / 7))) * 32767)
      points[i * 6] = -peak; points[i * 6 + 1] = peak
    }
    return bytes
  }
  if (command === 'list_tags') return tags.map(tag => ({
    ...tag, sampleCount: [...sampleTags.values()].filter(assigned => assigned.some(item => item.id === tag.id)).length,
  }))
  if (command === 'tag_samples') {
    const name = args.name?.trim() ?? ''
    let tag = tags.find(item => item.name.toLowerCase() === name.toLowerCase())
    if (!tag) {
      tag = { id: tags.length + 1, name }
      tags.push(tag)
    }
    for (const id of args.ids ?? []) {
      const assigned = sampleTags.get(id) ?? []
      if (!assigned.some(item => item.id === tag.id)) sampleTags.set(id, [...assigned, tag])
    }
    return tag
  }
  if (command === 'untag_samples') {
    for (const id of args.ids ?? []) {
      const assigned = sampleTags.get(id)?.filter(tag => tag.id !== args.tagId)
      if (assigned?.length) sampleTags.set(id, assigned)
      else sampleTags.delete(id)
    }
    return null
  }
  if (!['count_samples', 'list_samples', 'sample_ids'].includes(command)) return undefined
  const filter = args.filter
  const query = filter?.query?.trim().toLowerCase() ?? ''
  const matches = (id: number) => {
    const category = id % 2 ? 'kick' : 'snare'
    return (!filter?.sourceId || filter.sourceId === 1) && !filter?.favorites
      && (!filter?.category || filter.category === category)
      && (!filter?.tagId || sampleTags.get(id)?.some(tag => tag.id === filter.tagId))
      && (!query || `${category} ${id}`.includes(query) || sampleTags.get(id)?.some(tag => tag.name.toLowerCase().includes(query)))
  }
  // Scan integers, constructing objects only for the requested page.
  let ids: number[] = []
  for (let id = 1; id <= size; id++) if (matches(id)) ids.push(id)
  if (filter?.sortDirection === 'desc') ids.reverse()
  if (withStems) ids = ids.flatMap(id => id % 1000 === 0 && filter?.expandedStems?.includes(id) ? [id, -id * 10 - 1, -id * 10 - 2] : [id])
  if (command === 'count_samples') return ids.length
  if (command === 'sample_ids') return ids.slice(args.offset ?? 0,
    args.limit == null ? undefined : (args.offset ?? 0) + args.limit)
  const rows: Sample[] = ids.slice(args.offset ?? 0, (args.offset ?? 0) + (args.limit ?? 200)).map(id => ({
    id, sourceId: 1, name: `${id % 2 ? 'Kick' : 'Snare'} ${id}`, path: `/fixture/Pack ${id % 100}/Sample ${id}.wav`,
    ext: 'wav', size: 40000, duration: 0.5, sampleRate: 44100, channels: 2, bitDepth: 24,
    favorite: false, tags: sampleTags.get(id) ?? [], kind: 'oneshot', bpm: 120, bpmSource: 'audio', key: null, keySource: null,
    peakDb: -1, lufs: -18, analyzed: true, category: id % 2 ? 'kick' : 'snare', categorySource: 'audio',
    categoryConfidence: 1, similarity: null,
    stemCount: withStems && id > 0 && id % 1000 === 0 ? 2 : 0,
    ...(id < 0 ? { stemParentId: Math.floor(-id / 10), stemKind: -id % 10 === 1 ? 'vocals' as const : 'instrumental' as const } : {}),
  }))
  return new Promise(resolve => setTimeout(() => resolve(rows), 25))
}
