/** Bounded, non-reactive timings. Percentiles describe the most recent calls,
 * not the entire lifetime of an editor. No payloads or file paths are retained. */
export interface TimingSummary {
  name: string
  calls: number
  errors: number
  lastMs: number
  p50Ms: number
  p95Ms: number
  maxMs: number
}

export function createTimings(now: () => number = () => performance.now(), limit = 60) {
  const entries = new Map<string, { calls: number, errors: number, samples: number[] }>()
  let generation = 0
  function record(name: string, elapsed: number, failed: boolean, version: number) {
    if (version !== generation || !Number.isFinite(elapsed)) return
    let entry = entries.get(name)
    if (!entry) {
      if (entries.size === 32) entries.delete(entries.keys().next().value!)
      entry = { calls: 0, errors: 0, samples: [] }
      entries.set(name, entry)
    }
    entry.calls++
    if (failed) entry.errors++
    entry.samples.push(Math.max(0, elapsed))
    if (entry.samples.length > limit) entry.samples.shift()
  }
  async function measure<T>(name: string, work: () => Promise<T>): Promise<T> {
    const started = now(), version = generation
    let failed = true
    try { const result = await work(); failed = false; return result }
    finally { record(name, now() - started, failed, version) }
  }
  function measureSync<T>(name: string, work: () => T): T {
    const started = now(), version = generation
    let failed = true
    try { const result = work(); failed = false; return result }
    finally { record(name, now() - started, failed, version) }
  }
  function snapshot(): TimingSummary[] {
    return [...entries].map(([name, entry]) => {
      const sorted = [...entry.samples].sort((a, b) => a - b)
      const percentile = (p: number) => sorted[Math.max(0, Math.ceil(sorted.length * p) - 1)]!
      return { name, calls: entry.calls, errors: entry.errors, lastMs: entry.samples.at(-1)!,
        p50Ms: percentile(0.5), p95Ms: percentile(0.95), maxMs: sorted.at(-1)! }
    })
  }
  function reset() { generation++; entries.clear() }
  return { measure, measureSync, snapshot, reset }
}

export const editorTimings = createTimings()
