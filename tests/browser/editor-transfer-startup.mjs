// Exercise the standalone bootstrap with a pending native offer, even on a reload.
export async function checkEditorTransferStartup(page) {
  const check = (value, message) => { if (!value) throw new Error(message) }
  await page.waitForFunction(() => document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia?._s.get('editor')?.state)
  await page.evaluate(async () => {
    const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
    e.selection = { start: 10000, end: 90000 }
    e.gain(-3)
  })
  await page.waitForFunction(() => !document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').processing)
  await page.evaluate(async () => {
    const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
    await e.reset()
    const original = window.__TAURI_INTERNALS__.invoke
    window.__startupTransfer = { original, calls: [] }
    window.__TAURI_INTERNALS__.invoke = (cmd, args, options) => {
      const test = window.__startupTransfer
      test.calls.push(cmd)
      if (cmd === 'editor_accept_transfer') return new Promise(resolve => { test.accept = resolve })
      return original(cmd, args, options)
    }
    const url = new URL(location.href)
    const view = { cursor: 22000, selection: { start: 10000, end: 90000 }, looping: true, curve: 'exponential',
      viewport: { start: 10000, end: 90000, zoom: 2, bpm: 120, downbeat: 10 } }
    url.searchParams.set('editor', '1')
    url.searchParams.set('transfer', '42')
    url.searchParams.set('detached', '1')
    url.searchParams.set('view', JSON.stringify(view))
    history.replaceState(null, '', url)
    window.__startupTransfer.pending = e.init(1)
  })
  try {
    await page.waitForFunction(() => !!window.__startupTransfer.accept)
    check(await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const calls = window.__startupTransfer.calls
      return e.loading && e.sample && e.state.dirty && e.state.canUndo && e.cursor === 22000 && e.selection.end === 90000
        && !calls.includes('editor_open') && !calls.includes('editor_close') && calls.includes('editor_prepare_transfer')
    }), 'startup decoded/discarded audio or accepted before staging the UI')
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const test = window.__startupTransfer
      const before = test.calls.filter(cmd => cmd === 'editor_reject_transfer').length
      const duplicate = await e.adoptWindow(42, e.windowView)
      return !duplicate && test.calls.filter(cmd => cmd === 'editor_reject_transfer').length === before
    }), 'duplicate delivery cancelled its own pending transfer')
    await page.evaluate(async () => { window.__startupTransfer.accept(); await window.__startupTransfer.pending })
    check(await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const params = new URLSearchParams(location.search)
      return !e.loading && !e.error && e.state.canUndo && e.looping && e.curve === 'exponential'
        && e.windowView.viewport.downbeat === 10 && !params.has('transfer') && !params.has('detached') && !params.has('view') && params.get('editor') === '1'
    }), 'accepted startup did not restore the view or consume its token')
    return { passed: ['pending offer stages before acceptance', 'no decode/discard on transfer startup', 'history and view preserved', 'consumed token removed from URL', 'duplicate token does not cancel its pending transfer'] }
  } finally {
    await page.evaluate(() => {
      window.__TAURI_INTERNALS__.invoke = window.__startupTransfer.original
      const url = new URL(location.href)
      for (const key of ['editor', 'transfer', 'detached', 'view']) url.searchParams.delete(key)
      history.replaceState(null, '', url)
    })
  }
}
