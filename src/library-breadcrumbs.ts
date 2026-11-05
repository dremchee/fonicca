import type { Scope } from './library-scope.ts'

export interface LibraryBreadcrumb {
  label: string
  scope: Scope
  path?: string
}

/** Only the imported source and its descendants are library navigation targets. */
export function sourceBreadcrumbs(source: { id: number, path: string }, directory?: string): LibraryBreadcrumb[] {
  const trail: LibraryBreadcrumb[] = [{
    label: source.path.split(/[/\\]/).filter(Boolean).at(-1) ?? source.path,
    scope: { kind: 'source', id: source.id },
    path: source.path,
  }]
  const normalize = (path: string) => path.replaceAll('\\', '/').replace(/\/+$/, '') || '/'
  const root = normalize(source.path)
  const current = normalize(directory ?? source.path)
  const prefix = root.endsWith('/') ? root : `${root}/`
  if (current === root || !current.startsWith(prefix)) return trail
  const segments = current.slice(prefix.length).split('/').filter(Boolean)
  if (segments.some(segment => segment === '.' || segment === '..')) return trail
  const separator = source.path.includes('\\') && !source.path.includes('/') ? '\\' : '/'
  let path = source.path.replace(/[/\\]+$/, '')
  for (const label of segments) {
    path += separator + label
    trail.push({ label, path, scope: { kind: 'source', id: source.id, path } })
  }
  return trail
}
