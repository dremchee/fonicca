// Opt-in on /mock.html. Native session ownership/history are covered by Rust tests.
export async function checkEditorDetach(page) {
  const check = (v, message) => { if (!v) throw new Error(message) }
  await page.waitForFunction(() => document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia)
  await page.evaluate(async () => {
    const { useEditorStore } = await import('/src/stores/editor.ts')
    const editor = useEditorStore(document.querySelector('#app').__vue_app__.config.globalProperties.$pinia)
    await editor.openSample({ id: 1, name: 'Test', path: '/mock/Drum loop.wav', ext: 'wav', bpm: 120 })
    editor.selection = { start: 10000, end: 90000 }
    await editor.gain(-3)
    editor.cursor = 22000
    editor.looping = true
    editor.curve = 'exponential'
    const ipc = window.__TAURI_INTERNALS__, original = ipc.invoke
    window.__detachCheck = { original, requests: [], closed: 0 }
    ipc.invoke = (cmd, args, options) => {
      if (cmd === 'editor_detach_window') {
        window.__detachCheck.requests.push(args)
        return new Promise((resolve, reject) => { window.__detachCheck.resolve = resolve; window.__detachCheck.reject = reject })
      }
      if (cmd === 'editor_close') window.__detachCheck.closed++
      return original(cmd, args, options)
    }
  })
  const button = page.getByRole('button', { name: 'Open editor in a separate window', exact: true })
  try {
    await button.click()
    check(await button.isDisabled(), 'handoff must disable the button')
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return !await e.detachWindow() && window.__detachCheck.requests.length === 1
    }), 'duplicate handoff was permitted')
    await page.evaluate(() => window.__detachCheck.reject(new Error('Window creation failed')))
    await page.waitForFunction(() => !document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').loading)
    check(await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return e.state.dirty && e.state.canUndo && e.cursor === 22000 && e.looping && e.selection.start === 10000
    }), 'failed creation lost edits or selection')
    await button.click()
    await page.evaluate(() => window.__detachCheck.resolve())
    await button.waitFor({ state: 'hidden' })
    check(await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const v = window.__detachCheck.requests[1].view
      return !e.sample && !e.state && window.__detachCheck.closed === 0 && v.cursor === 22000 && v.looping && v.curve === 'exponential' && v.selection.end === 90000 && v.viewport.end > v.viewport.start
    }), 'handoff lost view state or closed the transferred native session')
    return { passed: ['toolbar action', 'duplicate guard', 'error recovery preserves edits/history/selection', 'retry', 'view transfer', 'source unmount without closing native session'] }
  } finally {
    await page.evaluate(() => { window.__TAURI_INTERNALS__.invoke = window.__detachCheck.original })
  }
}
