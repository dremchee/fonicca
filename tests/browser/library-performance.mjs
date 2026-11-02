// Development-browser observation on generated metadata; never opens a native library.
export async function measureLibraryPerformance(page, baseUrl, size = 32000) {
  const url = new URL(baseUrl)
  if (!['127.0.0.1', 'localhost'].includes(url.hostname) || url.pathname !== '/mock.html')
    throw new Error('Use the isolated local browser mock')
  url.searchParams.set('size', String(size))
  url.hash = 'library-large'
  await page.goto(url.href)
  await page.reload()
  await page.waitForFunction(expected => document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia._s.get('library')?.total === expected, size)
  await page.waitForSelector('[data-index] .track-title')
  const result = await page.evaluate(async () => {
    const pinia = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia
    const library = pinia._s.get('library'), player = pinia._s.get('player')
    const scroller = document.querySelector('.sample-list div[tabindex="0"].overflow-y-auto')
    const frames = [], longTasks = [], reads = [], updates = new Map()
    let activeReads = 0, maxReads = 0, maxRows = 0, maxPages = 0, maxScrollTop = 0, running = true, last
    const original = window.__TAURI_INTERNALS__.invoke
    window.__TAURI_INTERNALS__.invoke = async (command, args, options) => {
      if (command !== 'list_samples') return original(command, args, options)
      reads.push({ offset: args.offset, limit: args.limit })
      maxReads = Math.max(maxReads, ++activeReads)
      try { return await original(command, args, options) }
      finally { activeReads-- }
    }
    const observer = new PerformanceObserver(list => longTasks.push(...list.getEntries().map(e => e.duration)))
    if (PerformanceObserver.supportedEntryTypes.includes('longtask')) observer.observe({ type: 'longtask' })
    const tick = time => {
      if (!running) return
      if (last != null) frames.push(time - last)
      last = time
      maxRows = Math.max(maxRows, document.querySelectorAll('[data-index]').length)
      maxPages = Math.max(maxPages, library.pages.size)
      maxScrollTop = Math.max(maxScrollTop, scroller.scrollTop)
      requestAnimationFrame(tick)
    }
    requestAnimationFrame(tick)
    const wait = ms => new Promise(resolve => setTimeout(resolve, ms))
    const hooks = []
    try {
      await wait(200)
      const first = library.sampleAt(0)
      const { useWaveformBackgrounds } = await import('/src/composables/waveformBackgrounds.ts')
      const values = new Float32Array(256 * 6)
      for (let i = 0; i < values.length; i += 6) { values[i] = -.5; values[i + 1] = .5 }
      useWaveformBackgrounds().remember(first, { count: 256, lanes: 1, samples: false, stride: 6, values })
      player.playingId = first.id
      player.paused = false
      await wait(60)
      for (const row of document.querySelectorAll('[data-index]')) {
        let instance = row.__vueParentComponent
        while (instance && instance.type.name !== 'ReusableTemplate.reuse' && instance.type.__name !== 'SampleRow') instance = instance.parent
        if (!instance) continue
        const index = Number(row.dataset.index), previous = instance.u
        instance.u = [...(previous ?? []), () => updates.set(index, (updates.get(index) ?? 0) + 1)]
        hooks.push(() => { instance.u = previous })
      }
      const observedRows = hooks.length
      if (!observedRows) throw new Error('No library row components were observed')
      const started = performance.now()
      for (let frame = 0; frame < 120; frame++) {
        await new Promise(resolve => requestAnimationFrame(resolve))
        player.position = frame / 240
      }
      await wait(30)
      const playback = { observedRows, updates: [...updates].map(([index, count]) => ({ index, count })), elapsedMs: performance.now() - started }
      hooks.forEach(remove => remove())
      hooks.length = 0
      for (let step = 1; step <= 24; step++) {
        scroller.scrollTop = (scroller.scrollHeight - scroller.clientHeight) * (step / 24)
        await wait(100)
      }
      await wait(200)
      const pinned = document.querySelectorAll('[data-pinned="true"]').length
      scroller.scrollTop = 0
      await wait(200)
      const sorted = [...frames].sort((a, b) => a - b)
      const percentile = p => sorted[Math.max(0, Math.ceil(sorted.length * p) - 1)] ?? 0
      return { entries: library.total, playback, requests: reads, maxConcurrentReads: maxReads,
        maxRenderedRows: maxRows, maxScrollTop, maxRetainedPages: maxPages, retained: library.pageDiagnostics(), pinnedAtEnd: pinned,
        frameIntervalsMs: { p50: percentile(.5), p95: percentile(.95), max: sorted.at(-1) ?? 0 }, longTasksMs: longTasks }
    } finally {
      running = false
      observer.disconnect()
      hooks.forEach(remove => remove())
      window.__TAURI_INTERNALS__.invoke = original
      player.playingId = null
      player.position = 0
    }
  })
  if (!result.playback.updates.some(row => row.index === 0 && row.count > 0) || result.playback.updates.some(row => row.index !== 0 && row.count > 0))
    throw new Error(`Playback updated the wrong row components: ${JSON.stringify(result.playback)}`)
  if (result.maxScrollTop < (size - 100) * 36 || !result.requests.length || result.pinnedAtEnd !== 1)
    throw new Error(`Scroll scenario did not exercise pagination/pinning: ${JSON.stringify(result)}`)
  if (result.maxRetainedPages > 12 || result.maxConcurrentReads > 4 || result.maxRenderedRows > 100)
    throw new Error(`Unbounded library workload: ${JSON.stringify(result)}`)
  return result
}
