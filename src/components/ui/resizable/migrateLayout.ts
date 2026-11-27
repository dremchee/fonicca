import { resizableStorageKey } from '@ui-sdk/vue/resizable'

/** Reka sorts the panel IDs in its storage key. The sizes are in render order. */
export function migrateLayout(id: string, panelIds: string[][]) {
  try {
    const key = resizableStorageKey(id)
    if (localStorage.getItem(key) !== null) return
    const legacy = JSON.parse(localStorage.getItem(`reka:${id}`) ?? '{}')
    const layouts: Record<string, number[]> = {}
    for (const ids of panelIds) {
      const layout = legacy?.[[...ids].sort().join(',')]?.layout
      if (Array.isArray(layout) && layout.length === ids.length && layout.every(value => Number.isFinite(value))) {
        layouts[ids.join(',')] = layout
      }
    }
    if (Object.keys(layouts).length) localStorage.setItem(key, JSON.stringify(layouts))
  }
  catch {
    // Corrupt or unavailable preferences fall back to the panel defaults.
  }
}
