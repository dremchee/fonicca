import type { DuplicateGroup } from './api'

const normalize = (value: string) => value.normalize('NFKD').replace(/\p{M}/gu, '').toLowerCase()

/** Keep complete groups when any copy path matches every search term. */
export function matchingDuplicateGroups(groups: DuplicateGroup[], query: string) {
  const terms = normalize(query).trim().split(/\s+/).filter(Boolean)
  return terms.length ? groups.filter(group => group.copies.some(copy => {
    const path = normalize(copy.path)
    return terms.every(term => path.includes(term))
  })) : groups
}
