// Deliberately deliver stale success/failure IPC responses after reset. Native
// request cancellation is independently exercised by Rust lifecycle tests.
export async function checkEditorOpenCancellation(page) {
  const check = (value, message) => { if (!value) throw new Error(message) }
  await page.waitForFunction(() => {
    const p = document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia
    return p?._s.get('library')?.sources?.length && p._s.get('editor') && !p._s.get('editor').loading
  })
  await page.evaluate(() => {
    const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
    const original = window.__TAURI_INTERNALS__.invoke
    const test = window.__openCancellation = { original, requests: [], closes: [], deferClose: false, passThrough: false }
    window.__TAURI_INTERNALS__.invoke = (cmd, args, options) => {
      if (cmd === 'editor_open' && !args.reuse && !test.passThrough) {
        return new Promise((resolve, reject) => test.requests.push({ args, resolve, reject }))
      }
      if (cmd === 'editor_close') {
        test.closes.push(args)
        if (test.deferClose) return new Promise(resolve => { test.finishClose = resolve })
      }
      return original(cmd, args, options)
    }
    test.open = id => e.openSample({ id, name: `Track ${id}`, path: `/mock/Track ${id}.wav`, ext: 'wav', bpm: 120 })
    test.first = test.open(1)
  })
  try {
    await page.waitForFunction(() => window.__openCancellation.requests.length === 1)
    await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const test = window.__openCancellation
      test.deferClose = true
      test.reset = e.reset()
    })
    await page.waitForFunction(() => !!window.__openCancellation.finishClose)
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const test = window.__openCancellation
      test.requests[0].resolve(await test.original('editor_open', test.requests[0].args))
      await test.first
      return e.loading && !e.state
    }), 'stale success cleared reset loading or resurrected a track')
    await page.evaluate(async () => {
      const test = window.__openCancellation
      test.deferClose = false
      test.finishClose()
      await test.reset
    })
    check(await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return !e.sample && !e.state && !e.loading
    }), 'reset did not leave an empty editor')

    await page.evaluate(async () => {
      const test = window.__openCancellation
      test.passThrough = true
      await test.open(1)
      test.passThrough = false
      test.failed = test.open(2)
    })
    await page.waitForFunction(() => window.__openCancellation.requests.length === 2)
    await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const test = window.__openCancellation
      await e.reset()
      test.next = test.open(3)
    })
    await page.waitForFunction(() => window.__openCancellation.requests.length === 3)
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const test = window.__openCancellation
      test.requests[1].reject(new Error('editor opening cancelled'))
      await test.failed
      return e.loading && !e.state && e.sample.id === 3
    }), 'stale failure restored the old track or stopped the newer load')
    await page.evaluate(async () => {
      const test = window.__openCancellation
      test.requests[2].resolve(await test.original('editor_open', test.requests[2].args))
      await test.next
    })
    check(await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return !e.loading && e.sample.id === 3 && !!e.state
    }), 'newer load did not complete')
    check(await page.getByText('Error: editor opening cancelled', { exact: true }).count() === 0, 'stale cancellation produced an error toast')
    return { passed: ['reset invalidates late success', 'late success cannot clear reset loading', 'stale error cannot restore previous track', 'newer opening remains loading until its own response', 'stale cancellation is silent'] }
  } finally {
    await page.evaluate(() => { window.__TAURI_INTERNALS__.invoke = window.__openCancellation.original })
  }
}
