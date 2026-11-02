// Run against /mock.html#editor. Rust tests cover the live session ownership move.
export async function checkEditorAttach(page) {
  const check = (value, message) => { if (!value) throw new Error(message) }
  await page.waitForFunction(() => document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia._s.get('editor')?.state)
  await page.evaluate(() => {
    const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
    e.selection = { start: 10000, end: 90000 }
    e.gain(-3)
  })
  await page.waitForFunction(() => !document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').processing)
  await page.evaluate(() => {
    const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
    e.cursor = 22000
    e.looping = true
    e.curve = 'exponential'
    const original = window.__TAURI_INTERNALS__.invoke
    window.__attachCheck = { original, requests: [], destroyed: 0, closed: 0, prepared: 0, accepted: 0, rejected: 0 }
    window.__TAURI_INTERNALS__.invoke = (cmd, args, options) => {
      const test = window.__attachCheck
      if (cmd === 'editor_attach_window') {
        test.requests.push(args)
        return new Promise((resolve, reject) => { test.resolve = resolve; test.reject = reject })
      }
      if (cmd === 'plugin:window|destroy') { test.destroyed++; return Promise.resolve() }
      if (cmd === 'editor_close') test.closed++
      if (cmd === 'editor_prepare_transfer') test.prepared++
      if (cmd === 'editor_accept_transfer') {
        test.accepted++
        if (test.failAcceptance) return new Promise((resolve, reject) => { test.rejectAcceptance = reject })
      }
      if (cmd === 'editor_reject_transfer') test.rejected++
      return original(cmd, args, options)
    }
  })
  const button = page.locator('button[aria-label="Return editor to the main window"]')
  try {
    await button.click()
    check(await button.isDisabled(), 'return must be disabled during transfer')
    await page.evaluate(() => window.__attachCheck.reject(new Error('Main editor is occupied')))
    await page.waitForFunction(() => !document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').loading)
    check(await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return e.state.dirty && e.state.canUndo && e.selection.start === 10000 && !window.__attachCheck.destroyed
    }), 'failed return lost the source session')
    await button.click()
    await page.evaluate(() => window.__attachCheck.resolve())
    await page.waitForFunction(() => window.__attachCheck.destroyed === 1)
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const test = window.__attachCheck
      if (e.sample || e.state || test.closed) return false
      const view = test.requests[1].view
      await e.adoptWindow(1, view)
      return test.prepared === 1 && test.accepted === 1 && e.state.dirty && e.state.canUndo && e.cursor === 22000 && e.looping &&
        e.curve === 'exponential' && e.selection.end === 90000 && e.windowView.viewport.end > e.windowView.viewport.start
    }), 'adoption did not preserve view/history or re-used session')
    // A second offer cannot replace the editor already displayed in main.
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      const before = e.sample.id
      const accepted = await e.adoptWindow(2, window.__attachCheck.requests[1].view)
      return !accepted && e.sample.id === before && window.__attachCheck.prepared === 1 && window.__attachCheck.rejected === 1
    }), 'occupied UI accepted a second return')
    // During destination preparation both changing tracks and another offer
    // are blocked. An expired/failed commit must remove the staged editor UI.
    await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      await e.reset()
      const test = window.__attachCheck
      test.failAcceptance = true
      test.adoption = e.adoptWindow(3, test.requests[1].view)
    })
    await page.waitForFunction(() => !!window.__attachCheck.rejectAcceptance)
    check(await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return e.loading && e.sample && !await e.closeEmbedded() && !await e.adoptWindow(4, window.__attachCheck.requests[1].view)
    }), 'staged destination allowed another transition')
    check(await page.evaluate(async () => {
      const test = window.__attachCheck
      test.rejectAcceptance(new Error('Offer expired'))
      const adopted = await test.adoption
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return !adopted && !e.sample && !e.state && !e.loading && test.destroyed === 1
    }), 'failed commit left a stale editor UI or destroyed another window')
    return { passed: ['single return button', 'transfer guard', 'occupied destination keeps source', 'close source without discard', 'prepare then explicitly accept without decoding', 'occupied UI rejects a second offer', 'selection/viewport/history preserved', 'staged destination blocks competing transitions', 'expired acceptance removes staged UI'] }
  } finally {
    await page.evaluate(() => { window.__TAURI_INTERNALS__.invoke = window.__attachCheck.original })
  }
}
