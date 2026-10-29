/** Bounded page retention and request scheduling, independent of Vue and Tauri. */
interface Options<Row> {
  pages: Map<number, Row[]>
  fetch: (page: number) => Promise<Row[]>
  protectedPages?: () => number[]
  pageSize?: number
  maxPages?: number
  concurrency?: number
}
export interface LibraryPageDiagnostics {
  retainedPages: number
  retainedRows: number
  active: number
  queued: number
  maxPages: number
  concurrency: number
}
export function createLibraryPages<Row>(options: Options<Row>) {
  const { pages, fetch, pageSize = 200, maxPages = 12, concurrency = 4 } = options
  if (![pageSize, maxPages, concurrency].every(n => Number.isInteger(n) && n > 0)) throw new RangeError('invalid page cache limits')
  type Job = { page: number, version: number, required: boolean, refresh: boolean,
    promise: Promise<Row[] | undefined>, resolve: (rows: Row[] | undefined) => void, reject: (error: unknown) => void }
  const pending = new Map<number, Job>()
  const ages = new Map<number, number>()
  let viewport = new Set<number>(), version = 0, tick = 0, active = 0, disposed = false
  let queue: Job[] = []

  function trim() {
    const pinned = new Set((options.protectedPages?.() ?? []).slice(0, maxPages))
    const slots = maxPages - pinned.size
    if (slots > 0) for (const page of [...viewport].slice(-slots)) pinned.add(page)
    while (pages.size > maxPages) {
      const removable = [...pages.keys()].filter(page => !pinned.has(page)).sort((a, b) => (ages.get(a) ?? 0) - (ages.get(b) ?? 0))
      const oldest = removable[0]
      if (oldest === undefined) break
      pages.delete(oldest); ages.delete(oldest)
    }
  }
  function drain() {
    while (!disposed && active < concurrency && queue.length) {
      const job = queue.shift()!
      if (job.version !== version) { job.resolve(undefined); continue }
      active++
      const finish = () => {
        active--
        if (pending.get(job.page) === job) pending.delete(job.page)
        drain()
      }
      void Promise.resolve().then(() => fetch(job.page)).then(rows => {
        if (job.version !== version || disposed) { finish(); job.resolve(undefined); return }
        // A refresh must not bring back a page evicted while it was in flight.
        if (!job.refresh || pages.has(job.page)) {
          pages.set(job.page, rows); ages.set(job.page, ++tick); trim()
        }
        finish()
        job.resolve(rows)
      }, error => {
        finish()
        if (job.version === version && !disposed) job.reject(error)
        else job.resolve(undefined)
      })
    }
  }
  function load(page: number, required = true, refresh = false): Promise<Row[] | undefined> {
    if (disposed || !Number.isInteger(page) || page < 0) return Promise.resolve(undefined)
    const cached = pages.get(page)
    if (cached && !refresh) { ages.set(page, ++tick); return Promise.resolve(cached) }
    const existing = pending.get(page)
    if (existing) { existing.required ||= required; return existing.promise }
    let resolve!: Job['resolve'], reject!: Job['reject']
    const promise = new Promise<Row[] | undefined>((yes, no) => { resolve = yes; reject = no })
    const job: Job = { page, version, required, refresh, promise, resolve, reject }
    pending.set(page, job); queue.push(job); drain()
    return promise
  }
  function pageNumbers(start: number, end: number) {
    if (!Number.isFinite(start) || !Number.isFinite(end) || end < start || end < 0) return []
    return Array.from({ length: Math.floor(end / pageSize) - Math.floor(Math.max(0, start) / pageSize) + 1 },
      (_, i) => Math.floor(Math.max(0, start) / pageSize) + i)
  }
  async function ensureRange(start: number, end: number) {
    await Promise.all(pageNumbers(start, end).map(page => load(page)))
  }
  async function ensureVisibleRange(start: number, end: number) {
    viewport = new Set(pageNumbers(start, end))
    queue = queue.filter(job => {
      if (job.required || viewport.has(job.page) || options.protectedPages?.().includes(job.page)) return true
      if (pending.get(job.page) === job) pending.delete(job.page)
      job.resolve(undefined)
      return false
    })
    trim()
    await Promise.all([...viewport].map(page => load(page, false)))
  }
  async function refreshCached(page: number) {
    if (pages.has(page)) await load(page, false, true)
  }
  function clear() {
    version++; pages.clear(); ages.clear(); viewport.clear()
    for (const job of queue) job.resolve(undefined)
    queue = []; pending.clear()
  }
  function dispose() { disposed = true; clear() }
  const sampleAt = (index: number) => index >= 0 ? pages.get(Math.floor(index / pageSize))?.[index % pageSize] : undefined
  const diagnostics = () => ({ retainedPages: pages.size, retainedRows: [...pages.values()].reduce((sum, rows) => sum + rows.length, 0), active, queued: queue.length, maxPages, concurrency })
  trim()
  return { sampleAt, ensureRange, ensureVisibleRange, refreshCached, clear, dispose, diagnostics }
}
