import type { Sample, SampleFilter } from '@/api'
import coverUrl from '../../src-tauri/icons/128x128-dark.png?url'
import { stemSample } from './stems'

// Real UI / IPC boundary fixture. The app icon stands in for embedded artwork.
const samples: Sample[] = [
  { ...stemSample, name: 'Wee — Aeroplane (Reprise)', path: '/mock/Aeroplane.mp3', ext: 'mp3', duration: 167, bpm: 150.8, kind: 'long' },
  { ...stemSample, id: 2, name: 'Drum loop · no artwork', path: '/mock/Drum loop.wav' },
  { ...stemSample, id: 3, name: 'A very long sample name that should truncate without hiding the playback controls or the library search', path: '/mock/Long name.flac', ext: 'flac', duration: null },
]

export function headerPlayerDuration(id: number): number | null {
  return location.hash === '#header-player' ? samples.find(sample => sample.id === id)?.duration ?? null : null
}

export function headerPlayerCommand(command: string, args: { id?: number, filter?: SampleFilter, offset?: number, limit?: number }) {
  if (location.hash !== '#header-player') return undefined
  const matching = samples.filter(sample => !args.filter?.query || sample.name.toLowerCase().includes(args.filter.query.toLowerCase()))
  switch (command) {
    case 'list_sources': return [{ id: 1, path: '/mock/Dope', sampleCount: samples.length }]
    case 'source_folders': return []
    case 'count_samples': return matching.length
    case 'list_samples': return matching.slice(args.offset ?? 0, (args.offset ?? 0) + (args.limit ?? 200))
    case 'sample_ids': return matching.map(sample => sample.id)
    case 'sample_tags': return args.id === 1
      ? { title: 'Aeroplane (Reprise)', artist: 'Wee', album: null, year: null, genre: null, hasCover: true }
      : null
    case 'sample_cover':
      return args.id === 1
        ? fetch(coverUrl).then(response => response.arrayBuffer()).then(bytes => new Promise<ArrayBuffer>(resolve => {
            const delay = new URLSearchParams(location.search).has('slow-cover') ? 1200 : 0
            setTimeout(() => resolve(bytes), delay)
          }))
        : args.id === 3 ? Promise.reject(new Error('Artwork unavailable')) : new ArrayBuffer(0)
  }
}
