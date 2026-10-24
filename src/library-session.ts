import type { Collection, Source, SourceFolder, Tag } from './api'
import type { Scope } from './stores/library'

export type LibrarySessionScope = Exclude<Scope, { kind: 'similar' | 'duplicates' }>
export interface LibrarySession {
  scope: LibrarySessionScope
  sample: { id: number, index: number } | null
}

export function sessionScope(scope: Scope): LibrarySessionScope | null {
  if (scope.kind === 'similar' || scope.kind === 'duplicates')
    return null
  return { ...scope }
}

export function parseLibrarySession(value: unknown): LibrarySession | null {
  if (!value || typeof value !== 'object' || !('scope' in value))
    return null
  const scope = value.scope
  if (!scope || typeof scope !== 'object' || !('kind' in scope))
    return null
  let restored: LibrarySessionScope
  switch (scope.kind) {
    case 'all': case 'favorites': restored = { kind: scope.kind }; break
    case 'source': case 'tag': case 'collection': {
      if (!('id' in scope) || typeof scope.id !== 'number' || !Number.isSafeInteger(scope.id) || scope.id <= 0)
        return null
      restored = { kind: scope.kind, id: scope.id }
      if (scope.kind === 'source' && 'path' in scope && typeof scope.path === 'string' && scope.path)
        restored = { kind: 'source', id: scope.id, path: scope.path }
      break
    }
    default: return null
  }
  const sample = 'sample' in value ? value.sample : null
  const validSample = sample && typeof sample === 'object' && 'id' in sample && 'index' in sample
    && typeof sample.id === 'number' && Number.isSafeInteger(sample.id) && sample.id > 0
    && typeof sample.index === 'number' && Number.isSafeInteger(sample.index) && sample.index >= 0
  return { scope: restored, sample: validSample ? { id: sample.id as number, index: sample.index as number } : null }
}

function directoryPath(path: string) {
  return path.replaceAll('\\', '/').replace(/\/+$/, '') || '/'
}

export function isDirectoryWithin(path: string, root: string) {
  const directory = directoryPath(path)
  const parent = directoryPath(root)
  return directory === parent || directory.startsWith(parent.endsWith('/') ? parent : `${parent}/`)
}

export function resolveSessionScope(scope: LibrarySessionScope, sources: Source[], tags: Tag[], collections: Collection[], folders?: SourceFolder[]): LibrarySessionScope {
  if (scope.kind === 'source') {
    const source = sources.find(source => source.id === scope.id)
    if (!source)
      return { kind: 'all' }
    if (!scope.path || !isDirectoryWithin(scope.path, source.path) || (folders && !folders.some(folder => folder.path === scope.path)))
      return { kind: 'source', id: source.id }
  }
  if (scope.kind === 'tag' && !tags.some(tag => tag.id === scope.id))
    return { kind: 'all' }
  if (scope.kind === 'collection' && !collections.some(collection => collection.id === scope.id))
    return { kind: 'all' }
  return { ...scope }
}

/** The old row position is a cheap first try; IDs still find a track after sorting changes. */
export async function findSessionSampleIndex(
  sample: NonNullable<LibrarySession['sample']>,
  total: number,
  sampleIdAt: (index: number) => Promise<number | undefined>,
  listIds: () => Promise<number[]>,
): Promise<number> {
  if (total <= 0)
    return -1
  if (sample.index < total && await sampleIdAt(sample.index) === sample.id)
    return sample.index
  return (await listIds()).indexOf(sample.id)
}
