// Opt-in on the browser editor fixture. Tests actual canvas hit targets and undo.
export async function checkLoopResize(page) {
  const check = (value, message) => { if (!value) throw new Error(message) }
  await page.goto('http://127.0.0.1:1433/mock.html#editor')
  await page.reload()
  await page.waitForFunction(() => document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia?._s.get('editor')?.state?.frames > 0)
  await page.evaluate(async () => {
    const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
    const range = { start: Math.round(e.state.frames * .2), end: Math.round(e.state.frames * .6) }
    await e.setLoopPoints(range)
    e.selection = { ...range }
    e.looping = true; e.playing = true
    const original = window.__TAURI_INTERNALS__.invoke
    const width = document.querySelector('[data-editor-panel] canvas[tabindex="0"]').getBoundingClientRect().width
    window.__loopResize = { original, saved: [], updates: [], starts: 0, stops: 0, range, tolerance: e.state.frames / width * 2 }
    window.__TAURI_INTERNALS__.invoke = (cmd, args, options) => {
      if (cmd === 'editor_update_playback') { window.__loopResize.updates.push(args); return Promise.resolve(true) }
      if (cmd === 'editor_apply' && args.op.op === 'setLoop') window.__loopResize.saved.push(args.op)
      if (cmd === 'editor_play') window.__loopResize.starts++
      if (cmd === 'stop') window.__loopResize.stops++
      return original(cmd, args, options)
    }
  })
  try {
    const canvas = page.locator('[data-editor-panel] canvas[tabindex="0"]')
    const box = await canvas.boundingBox()
    const y = box.y + box.height * .6
    await page.keyboard.down('Alt') // temporarily bypass grid / zero snapping
    await page.mouse.move(box.x + box.width * .6, y)
    check(await canvas.evaluate(el => getComputedStyle(el).cursor) === 'ew-resize', 'loop edge does not expose a resize cursor')
    await page.mouse.down()
    await page.mouse.move(box.x + box.width * .75, y, { steps: 6 })
    check(await page.evaluate(() => window.__loopResize.saved.length === 0 && window.__loopResize.updates.length > 0), 'drag must preview playback without adding history on every move')
    await page.mouse.up()
    await page.keyboard.up('Alt')
    await page.waitForFunction(() => window.__loopResize.saved.length === 1 && !document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').processing)
    check(await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return e.state.loop.start === window.__loopResize.range.start && Math.abs(e.state.loop.end - e.state.frames * .75) < window.__loopResize.tolerance && e.playing && !window.__loopResize.starts && !window.__loopResize.stops
    }), 'resize changed the anchor or restarted playback')
    // The ruler grip must also work when no selection is present.
    await page.evaluate(() => { document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').selection = null })
    await page.keyboard.down('Alt')
    await page.mouse.move(box.x + box.width * .2, box.y + 16)
    await page.mouse.down()
    await page.mouse.move(box.x + box.width * .1, box.y + 16, { steps: 4 })
    await page.mouse.up()
    await page.keyboard.up('Alt')
    await page.waitForFunction(() => window.__loopResize.saved.length === 2 && !document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor').processing)
    check(await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return Math.abs(e.state.loop.start - e.state.frames * .1) < window.__loopResize.tolerance && Math.abs(e.state.loop.end - e.state.frames * .75) < window.__loopResize.tolerance
    }), 'ruler grip failed to resize the loop without a selection')
    await page.evaluate(async () => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      e.playing = false
      await e.undo()
    })
    check(await page.evaluate(() => {
      const e = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
      return e.state.loop.start === window.__loopResize.range.start && Math.abs(e.state.loop.end - e.state.frames * .75) < window.__loopResize.tolerance
    }), 'one undo did not restore exactly the preceding loop bounds')
    return { passed: ['wave edge hover/drag', 'live playback bounds', 'no restart', 'one undo step per drag', 'ruler grip without selection', 'fixed opposite boundary'] }
  } finally {
    await page.keyboard.up('Alt')
    await page.evaluate(() => { if (window.__loopResize) window.__TAURI_INTERNALS__.invoke = window.__loopResize.original })
  }
}
