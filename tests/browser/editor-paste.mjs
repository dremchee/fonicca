// Store/IPC integration on /mock.html#editor. Native conversion/history are
// verified separately by Rust tests; this check holds the paste response open.
export async function checkEditorPaste(page) {
  await page.waitForFunction(() => document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia?._s.get('editor')?.state?.frames > 0)
  const result = await page.evaluate(async () => {
    const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
    await editor.setMarkers([{ frame: 10, skip: false }]) // Make Undo available before testing its pending guard.
    const internals = window.__TAURI_INTERNALS__
    const original = internals.invoke
    const calls = []
    const check = (value, message) => { if (!value) throw new Error(message) }
    let gate
    let entered
    let pending
    const arm = () => {
      entered = new Promise(resolve => { gate = { entered: resolve } })
      pending = new Promise((resolve, reject) => { Object.assign(gate, { resolve, reject }) })
    }
    arm()
    internals.invoke = (command, args, options) => {
      calls.push(command)
      if (command !== 'editor_paste') return original(command, args, options)
      const current = gate
      const response = pending
      current.entered()
      return response.then(() => original(command, args, options))
    }
    try {
      editor.selection = { start: 1000, end: 2000 }
      await editor.copy()
      check(editor.clipboard?.frames === 1000, 'copy did not fill clipboard')
      const sample = editor.sample.path
      const first = editor.paste()
      await entered
      check(editor.processing, 'paste does not own the processing guard')
      await editor.paste()
      await editor.undo()
      await editor.gain(-1)
      check(!await editor.closeEmbedded(), 'editor closed while paste was pending')
      await editor.openSample({ id: 2, path: '/mock/Other.wav' })
      check(editor.sample.path === sample, 'pending paste allowed a session switch')
      check(calls.filter(command => command === 'editor_paste').length === 1, 'duplicate paste reached IPC')
      check(!calls.some(command => ['editor_apply', 'editor_undo', 'editor_close', 'editor_open'].includes(command)), 'an edit or transition reached IPC during paste')
      gate.resolve()
      await first
      check(!editor.processing, 'processing guard retained after success')
      check(editor.selection.start === 1000 && editor.selection.end === 2000 && editor.cursor === 1000, 'pasted selection/cursor changed')

      editor.selection = { start: 3000, end: 3500 }
      editor.cursor = 3200
      const before = JSON.stringify({ state: editor.state, selection: editor.selection, cursor: editor.cursor })
      arm()
      const failed = editor.paste()
      await entered
      gate.reject(new Error('paste fixture failure'))
      await failed
      check(!editor.processing, 'processing guard retained after failure')
      check(JSON.stringify({ state: editor.state, selection: editor.selection, cursor: editor.cursor }) === before, 'failed paste changed live state or selection')

      arm()
      const retry = editor.paste()
      await entered
      gate.resolve()
      await retry
      check(!editor.processing && editor.selection.start === 3000 && editor.selection.end === 4000, 'paste cannot retry after a failure')
      return { passed: ['copy', 'pending guard', 'duplicate paste', 'edit guard', 'session switch guard', 'selection and cursor', 'failure preserves state', 'retry'] }
    }
    finally { internals.invoke = original }
  })
  return result
}
